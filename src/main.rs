use clap::Parser;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
    execute, queue,
};
use std::env;
use rand::Rng;
use std::io::{self, Write};
use std::time::Duration;

// Grid structure for cellular automata
struct CaGrid {
    data: Vec<i32>,
    rows: usize,
    cols: usize,
}

impl CaGrid {
    fn new(rows: usize, cols: usize) -> Self {
        CaGrid {
            data: vec![0; rows * cols],
            rows,
            cols,
        }
    }

    fn idx(&self, row: usize, col: usize) -> i32 {
        self.data[row * self.cols + col]
    }

    fn set_idx(&mut self, row: usize, col: usize, val: i32) {
        self.data[row * self.cols + col] = val;
    }
}

// CLI arguments
#[derive(Parser)]
#[command(version, about = "A cozy fireplace in your terminal", long_about = None)]
struct Args {
    /// An ASCII character to draw the flames. Default is '@'
    #[arg(short = 'c', default_value = "@")]
    character: String,

    /// Set the framerate in frames/sec. Default is 20
    #[arg(short = 'f', default_value = "20")]
    framerate: i32,

    /// Set the maximum temperature of the flames. Default is 10
    #[arg(short = 't', default_value = "10")]
    temp: i32,

    /// Wolfram rule for flicker. Default is 60
    #[arg(short = 'w', default_value = "60")]
    wolfrule: u8,

    /// Print random characters
    #[arg(short = 'r')]
    random: bool,

    /// Disable black background
    #[arg(long)]
    no_background: bool,
}

// Global state
static mut PALETTE_SZ: usize = 0;
static mut WIDTH: usize = 0;
static mut HEIGHT: usize = 0;
static mut HEIGHTRECORD: usize = 0;
static mut USE_256_COLOR: bool = false;
static mut X256_PALETTE: [u8; 16] = [0; 16];

fn min(a: i32, b: i32) -> i32 {
    if a < b { a } else { b }
}

fn max(a: i32, b: i32) -> i32 {
    if a > b { a } else { b }
}

// Flip grid upside down for resize
fn flip_grid(grid: &mut CaGrid) {
    let rows = grid.rows;
    let cols = grid.cols;
    for i in 0..rows / 2 {
        for j in 0..cols {
            let temp = grid.idx(rows - i - 1, j);
            grid.set_idx(rows - i - 1, j, grid.idx(i, j));
            grid.set_idx(i, j, temp);
        }
    }
}

fn resize_array(ary: &mut Vec<u8>, new_size: usize) {
    let old_size = ary.len();
    let n = min(old_size as i32, new_size as i32) as usize;
    let mut temp = vec![0u8; new_size];
    for i in 0..n {
        temp[i] = ary[i];
    }
    *ary = temp;
}

// 256-color palette for flame gradient (from C version)
const X256: [u8; 16] = [
    233, 52, 88, 124,
    160, 166, 202, 208,
    214, 220, 226, 227,
    228, 229, 230, 231,
];

// Start terminal and initialize colors
fn start_crossterm(no_background: bool) -> io::Result<(usize, usize)> {
    let mut stdout = io::stdout();
    if no_background {
        execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    } else {
        execute!(stdout, EnterAlternateScreen, cursor::Hide, SetBackgroundColor(Color::Black))?;
    }
    terminal::enable_raw_mode()?;

    let (cols, rows) = terminal::size()?;
    let width = cols as usize;
    let height = rows as usize;

    // Detect terminal color support
    let use_256 = if env::consts::OS == "windows" {
        // Windows 10+ terminals (Windows Terminal, Console Host) support 256 colors
        true
    } else {
        env::var("TERM")
            .map(|term| term.contains("256color") || term.contains("truecolor"))
            .unwrap_or(false)
    };

    unsafe {
        if use_256 {
            USE_256_COLOR = true;
            X256_PALETTE = X256;
            PALETTE_SZ = 15; // 16 colors minus 1 (first is background)
        } else {
            USE_256_COLOR = false;
            PALETTE_SZ = 7;
        }
        WIDTH = width;
        HEIGHT = height;
    }

    Ok((width, height))
}

// Restore terminal
fn restore_terminal(no_background: bool) -> io::Result<()> {
    let mut stdout = io::stdout();
    if no_background {
        execute!(stdout, cursor::Show, LeaveAlternateScreen)?;
    } else {
        execute!(stdout, cursor::Show, LeaveAlternateScreen, ResetColor)?;
    }
    terminal::disable_raw_mode()?;
    Ok(())
}

// Cooldown function
fn cooldown(heat: i32) -> i32 {
    if heat == 0 {
        return 0;
    }
    let mut rng = rand::thread_rng();
    let r = rng.gen_range(0..heat);
    if r == 0 {
        heat - 1
    } else {
        heat
    }
}

// Clear grid above height
fn cleargrid(grid: &mut CaGrid, h: usize) {
    let height = unsafe { HEIGHT };
    for i in h..height {
        for j in 0..grid.cols {
            grid.set_idx(i, j, 0);
        }
    }
}

// Warm the hotplate
fn warm(heater: &[u8], hotplate: &mut [u8], maxtemp: i32) {
    for i in 0..hotplate.len() {
        hotplate[i] /= 2;
    }
    for i in 0..hotplate.len() {
        hotplate[i] = hotplate[i].saturating_add((heater[i] as i32 * maxtemp) as u8);
    }
}

// Next frame of cellular automata
fn nextframe(field: &mut CaGrid, count: &mut CaGrid, hotplate: &[u8]) {
    let height = unsafe { HEIGHT };
    let width = unsafe { WIDTH };
    let mut heightrecord = unsafe { HEIGHTRECORD };

    cleargrid(count, heightrecord);

    let h = max(heightrecord as i32 - 3, 1) as usize;

    for i in h..=height {
        let mut rowsum = 0;
        for j in 0..width {
            let mut avg = 0.0;
            let mut counter = 0;

            for xoff in -3..=3 {
                for yoff in -1..=3 {
                    let y = i as i32 + yoff;
                    let y = max(y, 0) as usize;
                    let x = j as i32 + xoff;

                    if x < 0 || x >= width as i32 {
                        avg += 0.0;
                    } else if y >= height {
                        avg += hotplate[x as usize] as f32;
                    } else {
                        avg += field.idx(y, x as usize) as f32;
                    }
                    counter += 1;
                }
            }

            avg /= counter as f32;
            let val = cooldown(avg as i32);
            if i > 0 {
                count.set_idx(i - 1, j, val);
            }
            rowsum += val;
        }
        if rowsum > 0 && i < heightrecord {
            heightrecord = i;
        }
    }

    // Copy count to field
    for i in 0..height {
        for j in 0..width {
            field.set_idx(i, j, count.idx(i, j));
        }
    }

    unsafe {
        HEIGHTRECORD = heightrecord;
    }
}

// Wolfram's Elementary cellular automaton
fn wolfram(world: &mut [u8], rule: u8) {
    let width = unsafe { WIDTH };
    let mut next = vec![0u8; width];

    for i in 0..width {
        let lidx = if i > 0 { i - 1 } else { width - 1 };
        let ridx = (i + 1) % width;
        let l = world[lidx];
        let c = world[i];
        let r = world[ridx];
        let current = ((l as usize) << 2) | ((c as usize) << 1) | (r as usize);
        next[i] = (rule >> current) & 0b1;
    }

    world.copy_from_slice(&next);
}

// Print frame to terminal
fn printframe(
    field: &CaGrid,
    dispch: char,
    maxtemp: i32,
    random_mode: bool,
    no_background: bool,
) -> io::Result<()> {
    let mut stdout = io::stdout();
    let heightrecord = unsafe { HEIGHTRECORD };
    let height = unsafe { HEIGHT };
    let width = unsafe { WIDTH };
    let palette_sz = unsafe { PALETTE_SZ };
    let use_256 = unsafe { USE_256_COLOR };

    let char_list = ['@', '#', '%', '&', '*', '+', '=', '-', '~', '^'];
    let char_list_size = char_list.len();

    for i in heightrecord..height {
        for j in 0..width {
            let cell = field.idx(i, j);
            if cell == 0 {
                if no_background {
                    queue!(stdout, cursor::MoveTo(j as u16, i as u16), Print(' '))?;
                } else {
                    queue!(stdout, cursor::MoveTo(j as u16, i as u16), SetBackgroundColor(Color::Black), Print(' '))?;
                }
            } else {
                let color_idx = min(palette_sz as i32, (palette_sz as i32 * cell / maxtemp) + 1) as usize;

                let color = if use_256 {
                    // Use 256-color palette (x256 gradient)
                    let idx = unsafe { X256_PALETTE[color_idx] };
                    Color::AnsiValue(idx as u8)
                } else {
                    // Fallback to basic colors
                    match color_idx {
                        1 => Color::DarkGrey,
                        2 => Color::DarkRed,
                        3 => Color::Red,
                        4 => Color::DarkYellow,
                        5 => Color::Yellow,
                        6 => Color::White,
                        7 => Color::White,
                        _ => Color::Black,
                    }
                };

                let ch = if random_mode {
                    char_list[rand::thread_rng().gen_range(0..char_list_size)]
                } else {
                    dispch
                };

                if no_background {
                    queue!(
                        stdout,
                        cursor::MoveTo(j as u16, i as u16),
                        SetForegroundColor(color),
                        Print(ch),
                    )?;
                } else {
                    queue!(
                        stdout,
                        cursor::MoveTo(j as u16, i as u16),
                        SetForegroundColor(color),
                        SetBackgroundColor(Color::Black),
                        Print(ch),
                    )?;
                }
            }
        }
    }

    stdout.flush()?;
    Ok(())
}

// Main flames function
fn flames(
    dispch: char,
    wolfrule: u8,
    mut maxtemp: i32,
    frameperiod: Duration,
    random_mode: bool,
    no_background: bool,
) -> io::Result<()> {
    let width = unsafe { WIDTH };
    let height = unsafe { HEIGHT };

    let mut field = CaGrid::new(height, width);
    let mut count = CaGrid::new(height, width);

    let mut heater: Vec<u8> = vec![0; width];
    let mut hotplate: Vec<u8> = vec![0; width];

    let mut rng = rand::thread_rng();
    for i in 0..width {
        heater[i] = rng.gen_range(0..2);
    }

    loop {
        // Check for keypress
        if event::poll(Duration::from_millis(0))? {
            if let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? {
                match code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => break,
                    KeyCode::Char('c') if modifiers.contains(event::KeyModifiers::CONTROL) => break,
                    KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => maxtemp += 1,
                    KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                        if maxtemp > 1 {
                            maxtemp -= 1;
                        }
                    }
                    _ => {}
                }
            }
        }

        // Check for terminal resize
        if event::poll(Duration::from_millis(0))? {
            if let Event::Resize(cols, rows) = event::read()? {
                unsafe {
                    HEIGHTRECORD = 0;
                    WIDTH = cols as usize;
                    HEIGHT = rows as usize;

                    resize_array(&mut heater, WIDTH);
                    resize_array(&mut hotplate, WIDTH);

                    flip_grid(&mut field);
                    flip_grid(&mut count);
                    field = CaGrid::new(HEIGHT, WIDTH);
                    count = CaGrid::new(HEIGHT, WIDTH);
                    flip_grid(&mut field);
                    flip_grid(&mut count);
                }
            }
        }

        wolfram(&mut heater, wolfrule);

        // Random heater flip
        if rng.gen_range(0..30) == 0 {
            heater[rng.gen_range(0..width)] ^= 0x1;
        }

        warm(&heater, &mut hotplate, maxtemp);
        printframe(&field, dispch, maxtemp, random_mode, no_background)?;
        nextframe(&mut field, &mut count, &hotplate);

        std::thread::sleep(frameperiod);
    }

    Ok(())
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let dispch = args.character.chars().next().unwrap_or('@');
    let wolfrule = args.wolfrule;
    let maxtemp = args.temp;
    let frameperiod = if args.framerate < 1 {
        Duration::from_secs(0)
    } else {
        Duration::from_micros(1_000_000 / args.framerate as u64)
    };
    let random_mode = args.random;
    let no_background = args.no_background;

    let (width, height) = start_crossterm(no_background)?;
    unsafe {
        WIDTH = width;
        HEIGHT = height;
    }

    flames(dispch, wolfrule, maxtemp, frameperiod, random_mode, no_background)?;

    restore_terminal(no_background)?;

    // Clear screen on exit
    let mut stdout = io::stdout();
    execute!(stdout, Clear(ClearType::All))?;

    Ok(())
}
