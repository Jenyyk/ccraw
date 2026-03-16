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
const MAX_CROW_SPEED: f32 = 4.0;

#[derive(Debug)]
struct Game {
    term_height: u16,
    term_width: u16,
    crows: Vec<Crow>,
    variants: Vec<CrowVariant>,
    max_crows: usize,
    last_event: String,
}

impl Default for Game {
    fn default() -> Self {
        Self {
            term_height: 0,
            term_width: 0,
            crows: Vec::new(),
            variants: Vec::new(),
            max_crows: 5,
            last_event: String::new(),
        }
    }
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
        let mut speed = (rng.random_range(1.0..3.5), rng.random_range(-1.2..1.2));
        if rng.random_bool(0.5) {
            speed.0 = -speed.0;
        }
        let acceleration = (rng.random_range(-0.3..0.3), rng.random_range(-0.1..0.1));
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
                    (self.position.1 as u16 + num as u16).saturating_sub(1),
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

    // load settings
    let mut game = Game {
        variants: crow_variants,
        ..Default::default()
    };
    load_args(&mut game);

    // prepare terminal
    let mut stdout = stdout();
    game.refresh_terminal_info()?;
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    enable_raw_mode()?;

    // prepare error handling
    let panic_info = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
    let panic_info_clone = panic_info.clone();

    std::panic::set_hook(Box::new(move |info| {
        let bt = std::backtrace::Backtrace::capture();
        *panic_info_clone.lock().unwrap() = Some(format!("{info}\n{bt}"));
    }));

    //game loop
    let result = std::panic::catch_unwind(move || {
        loop {
            let frame_start_time = std::time::Instant::now();
            let frame_end_time = frame_start_time + std::time::Duration::from_secs(1) / FPS;

            game.update();
            game.refresh_terminal_info().unwrap();
            game.clear_old_crows();
            while game.crows.len() < game.max_crows {
                game.add_crow(game.create_crow());
            }
            queue!(stdout, Clear(crossterm::terminal::ClearType::All)).unwrap();
            for crow in &game.crows {
                crow.draw(&mut stdout).unwrap();
            }
            // if in debug mode, print events to screen
            #[cfg(debug_assertions)]
            {
                queue!(
                    stdout,
                    MoveTo(0, game.term_height.saturating_sub(1)),
                    Print(&game.last_event)
                )
                .unwrap();
            }
            stdout.flush().unwrap();

            if poll(frame_end_time - std::time::Instant::now()).unwrap()
                && let Event::Key(e) = read().unwrap()
            {
                game.last_event = format!("{:?}", e);
                handle_events(e, &mut game);
            }
        }
    });

    cleanup();

    if result.is_err() {
        if let Some(msg) = panic_info.lock().unwrap().take() {
            eprintln!("{msg}");
        }
        std::process::exit(1);
    }

    Ok(())
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

fn handle_events(event: KeyEvent, game: &mut Game) {
    match event.code {
        // quitting
        KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => {
            graceful_exit();
        }
        KeyCode::Char('q') => {
            graceful_exit();
        }

        // controls
        KeyCode::Char('+') => {
            game.max_crows += 1;
        }
        KeyCode::Char('-') => {
            game.max_crows = game.max_crows.saturating_sub(1);
        }
        _ => {}
    }
}

fn graceful_exit() {
    cleanup();
    std::process::exit(0);
}

fn cleanup() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show);
}

fn load_args(game: &mut Game) {
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--crows" => game.max_crows = args.next().and_then(|s| s.parse().ok()).unwrap_or(game.max_crows),
            _ => eprintln!("Invalid argument: {arg}"),
        }
    }
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
