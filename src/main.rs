use clap::Parser;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    execute, queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use rand::Rng;
use std::env;
use std::io::{self, Write};
use std::time::Duration;

// Grid structure for cellular automata
struct CaGrid {
    data: Vec<i32>,
    rows: usize,
    cols: usize,
}

fn is_east_asian_wide(c: char) -> bool {
    let cp = c as u32;
    (0x1100..=0x115F).contains(&cp)
        || matches!(c, '\u{2329}' | '\u{232A}')
        || (0x2E80..=0x2EFF).contains(&cp)
        || (0x3000..=0x303F).contains(&cp)
        || (0x3040..=0x309F).contains(&cp)
        || (0x30A0..=0x30FF).contains(&cp)
        || (0x3100..=0x312F).contains(&cp)
        || (0x3130..=0x318F).contains(&cp)
        || (0x3190..=0x319F).contains(&cp)
        || (0x31A0..=0x31BF).contains(&cp)
        || (0x31C0..=0x31EF).contains(&cp)
        || (0x31F0..=0x31FF).contains(&cp)
        || (0x3200..=0x32FF).contains(&cp)
        || (0x3300..=0x335F).contains(&cp)
        || (0x3370..=0x33FF).contains(&cp)
        || (0xA000..=0xA49F).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0xFE10..=0xFE1F).contains(&cp)
        || (0xFE30..=0xFE6F).contains(&cp)
        || (0xFF00..=0xFF60).contains(&cp)
        || (0xFFE0..=0xFFE6).contains(&cp)
        || (0x1F300..=0x1F64F).contains(&cp)
        || (0x1F680..=0x1F6FF).contains(&cp)
        || (0x1F900..=0x1FAFF).contains(&cp)
        || (0x2600..=0x26FF).contains(&cp)
        || (0x2700..=0x27BF).contains(&cp)
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
    #[arg(short = 'c', default_value = "@", conflicts_with_all = ["use_cool_unicode"])]
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

    /// Use decorative unicode (1: 🮿, 2: 𜵯, 3: 🮋, 4: 𜺏)
    #[arg(short = 'u', long = "use-cool-unicode")]
    use_cool_unicode: bool,

    /// Unicode character number (1-4)
    #[arg(short = 'n', long = "unicode-num", default_value = "1")]
    unicode_num: u8,
}

// Global state
static mut PALETTE_SZ: usize = 0;
static mut WIDTH: usize = 0;
static mut HEIGHT: usize = 0;
static mut HEIGHTRECORD: usize = 0;
static mut USE_256_COLOR: bool = false;
static mut USE_TRUECOLOR: bool = false;
static mut X256_PALETTE: [u8; 16] = [0; 16];
static mut WIDE_COLS: Vec<u8> = Vec::new();

fn min(a: i32, b: i32) -> i32 {
    if a < b {
        a
    } else {
        b
    }
}

fn max(a: i32, b: i32) -> i32 {
    if a > b {
        a
    } else {
        b
    }
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

fn ensure_wide_cols(width: usize) {
    #[allow(static_mut_refs)]
    unsafe {
        let wide_cols = &mut WIDE_COLS;
        if wide_cols.len() < width {
            wide_cols.resize(width, 0);
        }
    }
}

#[allow(static_mut_refs)]
fn wide_cols_at(idx: usize) -> Option<&'static mut u8> {
    unsafe {
        let cols = &mut WIDE_COLS;
        cols.get_mut(idx)
    }
}

// 256-color palette for flame gradient (from C version)
const X256: [u8; 16] = [
    233, 52, 88, 124, 160, 166, 202, 208, 214, 220, 226, 227, 228, 229, 230, 231,
];

// Start terminal and initialize colors
fn start_crossterm(no_background: bool) -> io::Result<(usize, usize)> {
    let mut stdout = io::stdout();

    terminal::enable_raw_mode()?;

    let (cols, rows) = terminal::size()?;
    let width = cols as usize;
    let height = rows as usize;

    // Detect terminal color support
    let use_256 = if env::consts::OS == "windows" {
        true
    } else {
        env::var("TERM")
            .map(|term| term.contains("256color") || term.contains("truecolor"))
            .unwrap_or(false)
    };
    let use_truecolor = if env::consts::OS == "windows" {
        true
    } else {
        env::var("COLORTERM")
            .map(|ct| ct.contains("truecolor") || ct.contains("24bit"))
            .unwrap_or(false)
            || env::var("TERM")
                .map(|term| term.contains("truecolor") || term.contains("24bit"))
                .unwrap_or(false)
    };

    unsafe {
        if use_256 {
            USE_256_COLOR = true;
            X256_PALETTE = X256;
            PALETTE_SZ = 15;
        } else {
            USE_256_COLOR = false;
            PALETTE_SZ = 7;
        }
        USE_TRUECOLOR = use_truecolor;
        WIDTH = width;
        HEIGHT = height;
        ensure_wide_cols(width);
    }

    if no_background {
        execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    } else if use_truecolor {
        execute!(
            stdout,
            EnterAlternateScreen,
            cursor::Hide,
            SetBackgroundColor(Color::Rgb { r: 0, g: 0, b: 0 })
        )?;
    } else {
        execute!(
            stdout,
            EnterAlternateScreen,
            cursor::Hide,
            SetBackgroundColor(Color::AnsiValue(16))
        )?;
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
    let wide_char = is_east_asian_wide(dispch);

    let char_list = ['@', '#', '%', '&', '*', '+', '=', '-', '~', '^'];
    let char_list_size = char_list.len();

    for i in heightrecord..height {
        let mut j = 0;
        while j < width {
            let cell = field.idx(i, j);

            if cell == 0 {
                if wide_char && j + 1 < width {
                    if let Some(slot) = wide_cols_at(j) {
                        *slot = 0;
                    }
                }
                if no_background {
                    queue!(stdout, cursor::MoveTo(j as u16, i as u16), Print(' '))?;
                } else {
                    let bg = if unsafe { USE_TRUECOLOR } {
                        Color::Rgb { r: 0, g: 0, b: 0 }
                    } else {
                        Color::AnsiValue(16)
                    };
                    queue!(
                        stdout,
                        cursor::MoveTo(j as u16, i as u16),
                        SetBackgroundColor(bg),
                        Print(' ')
                    )?;
                }
            } else {
                let color_idx =
                    min(palette_sz as i32, (palette_sz as i32 * cell / maxtemp) + 1) as usize;

                let color = if use_256 {
                    let idx = unsafe { X256_PALETTE[color_idx] };
                    Color::AnsiValue(idx as u8)
                } else {
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
                    let bg = if unsafe { USE_TRUECOLOR } {
                        Color::Rgb { r: 0, g: 0, b: 0 }
                    } else {
                        Color::AnsiValue(16)
                    };
                    queue!(
                        stdout,
                        cursor::MoveTo(j as u16, i as u16),
                        SetForegroundColor(color),
                        SetBackgroundColor(bg),
                        Print(ch),
                    )?;
                }
            }

            if wide_char && cell > 0 && j + 1 < width {
                if let Some(slot) = wide_cols_at(j) {
                    *slot = 1;
                }
                j += 2;
            } else {
                j += 1;
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
            if let Event::Key(KeyEvent {
                code, modifiers, ..
            }) = event::read()?
            {
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
                    ensure_wide_cols(WIDTH);

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

    let dispch = if args.use_cool_unicode {
        match args.unicode_num {
            1 => '🮿',
            2 => '𜵯',
            3 => '🮋',
            4 => '𜺏',
            _ => '🮿',
        }
    } else {
        args.character.chars().next().unwrap_or('@')
    };
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

    flames(
        dispch,
        wolfrule,
        maxtemp,
        frameperiod,
        random_mode,
        no_background,
    )?;

    restore_terminal(no_background)?;

    // Clear screen on exit
    let mut stdout = io::stdout();
    execute!(stdout, Clear(ClearType::All))?;

    Ok(())
}
