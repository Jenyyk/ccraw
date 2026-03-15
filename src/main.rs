use crossterm::{
    cursor::{self, MoveTo},
    event::{Event, KeyCode, KeyEvent, KeyModifiers, poll, read},
    execute, queue,
    style::Print,
    terminal::{
        Clear, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
    },
};
use rand::RngExt;
use std::io::{self, Write, stdout};

const FPS: u32 = 6;
const MAX_CROWS: usize = 5;
const MAX_CROW_SPEED: f32 = 4.0;

#[derive(Debug, Default)]
struct Game {
    term_height: u16,
    term_width: u16,
    crows: Vec<Crow>,
    variants: Vec<CrowVariant>,
}

impl Game {
    fn update(&mut self) {
        for crow in self.crows.iter_mut() {
            crow.position.0 += crow.speed.0;
            crow.position.1 += crow.speed.1;
            crow.speed.0 = f32::clamp(
                crow.speed.0 + crow.acceleration.0,
                -MAX_CROW_SPEED,
                MAX_CROW_SPEED,
            );
            crow.speed.1 = f32::clamp(
                crow.speed.1 + crow.acceleration.1,
                -MAX_CROW_SPEED,
                MAX_CROW_SPEED,
            );
            crow.current_frame += 1;
        }
    }

    fn refresh_terminal_info(&mut self) -> Result<(), std::io::Error> {
        let terminal_size = crossterm::terminal::size()?;
        self.term_height = terminal_size.1;
        self.term_width = terminal_size.0;
        Ok(())
    }

    fn clear_old_crows(&mut self) {
        self.crows.retain(|crow| {
            !(crow.position.0 > self.term_width as f32
                || (crow.position.0 + crow.variant.width as f32) < 0.0
                || crow.position.1 > self.term_height as f32
                || (crow.position.1 + crow.variant.height as f32) < 0.0)
        })
    }

    fn add_crow(&mut self, crow: Crow) {
        self.crows.push(crow);
    }

    fn create_crow(&self) -> Crow {
        let mut rng = rand::rng();
        let variant = self.variants[rng.random_range(0..self.variants.len())].clone();
        let speed = (rng.random_range(-3.0..3.0), rng.random_range(-1.2..1.2));
        let acceleration = (rng.random_range(-0.4..0.4), rng.random_range(-0.1..0.1));
        let spawn_y = rng.random_range(3..self.term_height - 3);
        let position = if speed.0 > 0.0 {
            (-(variant.width as f32) + 0.2, spawn_y as f32)
        } else {
            (self.term_width as f32, spawn_y as f32)
        };

        Crow {
            variant,
            current_frame: 0,
            position,
            speed,
            acceleration,
        }
    }
}

#[derive(Clone, Debug)]
struct CrowVariant {
    width: usize,
    height: usize,
    frames: Vec<Vec<String>>,
    total_frames: usize,
}

#[derive(Debug)]
struct Crow {
    variant: CrowVariant,
    current_frame: usize,
    position: (f32, f32),
    speed: (f32, f32),
    acceleration: (f32, f32),
}

impl Crow {
    fn draw(&self, stdout: &mut io::Stdout) -> Result<(), std::io::Error> {
        let cur_frame = &self.variant.frames[self.current_frame % self.variant.total_frames];
        for (num, line) in cur_frame.iter().enumerate() {
            queue!(
                stdout,
                MoveTo(
                    self.position.0 as u16,
                    self.position.1 as u16 + num as u16 - 1
                ),
                Print(line)
            )?;
        }
        Ok(())
    }
}

fn main() -> Result<(), std::io::Error> {
    // load crow art
    let crowfile = include_str!("../compiled_crows.txt");
    let crow_variants: Vec<CrowVariant> = parse_crowfile(crowfile);

    // prepare
    let mut game = Game {
        variants: crow_variants,
        ..Default::default()
    };
    let mut stdout = stdout();
    game.refresh_terminal_info()?;
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    enable_raw_mode()?;

    loop {
        game.update();
        game.refresh_terminal_info()?;
        game.clear_old_crows();
        if game.crows.len() < MAX_CROWS {
            game.add_crow(game.create_crow());
        }
        queue!(stdout, Clear(crossterm::terminal::ClearType::All))?;
        for crow in &game.crows {
            crow.draw(&mut stdout)?;
        }
        stdout.flush()?;
        if poll(std::time::Duration::from_secs(1) / FPS)?
            && let Event::Key(e) = read()?
        {
            handle_events(e);
        }
    }
}

fn parse_crowfile(crowfile: &str) -> Vec<CrowVariant> {
    let chars = crowfile.chars();

    let mut crows = Vec::new();
    let mut frames = Vec::new();
    let mut lines = Vec::new();
    let mut line = String::new();

    for ch in chars {
        match ch {
            'n' => {
                lines.push(std::mem::take(&mut line));
            }
            'f' => {
                frames.push(std::mem::take(&mut lines));
            }
            'c' => {
                crows.push(CrowVariant {
                    height: frames.iter().map(|f| f.len()).max().unwrap_or(0),
                    width: frames
                        .iter()
                        .map(|f| f.iter().map(|l| l.len()).max().unwrap_or(0))
                        .max()
                        .unwrap_or(0),
                    total_frames: frames.len(),
                    frames: std::mem::take(&mut frames),
                });
            }
            _ => {
                line.push(ch);
            }
        }
    }

    crows
}

fn handle_events(event: KeyEvent) {
    if (event.code == KeyCode::Char('c') && event.modifiers.contains(KeyModifiers::CONTROL))
        || event.code == KeyCode::Char('q')
    {
        graceful_exit();
    }
}

fn graceful_exit() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show);
    std::process::exit(0);
}

mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn parsing_crowfile() {
        let crows = parse_crowfile(r"\_/nf _n/ \nfc");
        assert_eq!(crows.len(), 1);
        let crow = &crows[0];
        assert_eq!(crow.height, 2);
        assert_eq!(crow.width, 3);
    }
}
