use ratatui::prelude::Color;

#[derive(Debug, Clone)]
pub struct Theme {
    //bg colors
    pub primary_bg: Color,
    pub secondary_bg: Color,

    //text colors
    pub primary_text: Color,
    pub secondary_text: Color,
    pub highlight_text: Color,
    pub warning_text: Color,
    pub error_text: Color,

    //border colors
    pub primary_border: Color,
    pub secondary_border: Color,
    pub active_border: Color,

    //gauge colors
    pub gauge_primary: Color,
    pub gauge_secondary: Color,
    pub gauge_background: Color,

    //cpu usage colors
    pub cpu_low: Color,
    pub cpu_medium: Color,
    pub cpu_high: Color,
    pub cpu_critical: Color,

    //mem. colors
    pub memory_normal: Color,
    pub memory_warning: Color,
    pub memory_critical: Color,

    //proc. colors
    pub process_normal: Color,
    pub process_selected: Color,
    pub process_high_cpu: Color,
    pub process_info: Color,

    //network colors
    pub network_border: Color,

    //animation color
    pub animation: Color,
}

impl Theme {
    //default b-top theme (this is beyond manual)
    pub fn dark_purple() -> Self {
        Self {
            primary_bg: Color::Rgb(31, 3, 64),
            secondary_bg: Color::Black,

            primary_text: Color::White,
            secondary_text: Color::Gray,
            highlight_text: Color::LightCyan,
            warning_text: Color::LightYellow,
            error_text: Color::Red,

            primary_border: Color::Rgb(126, 48, 219),
            secondary_border: Color::Rgb(137, 125, 219),
            active_border: Color::Cyan,

            gauge_primary: Color::Rgb(126, 48, 219),
            gauge_secondary: Color::Rgb(137, 125, 219),
            gauge_background: Color::Rgb(31, 3, 64),

            cpu_low: Color::Rgb(126, 207, 126),
            cpu_medium: Color::Yellow,
            cpu_high: Color::Rgb(255, 165, 0),
            cpu_critical: Color::Red,

            memory_normal: Color::Rgb(126, 48, 219),
            memory_warning: Color::Yellow,
            memory_critical: Color::Red,

            process_normal: Color::White,
            process_selected: Color::LightCyan,
            process_high_cpu: Color::Red,
            process_info: Color::Cyan,

            network_border: Color::Cyan,

            animation: Color::Rgb(64, 64, 64),
        }
    }

    //yeah -> replaced with monotone
    pub fn monotone() -> Self {
        Self {
            primary_bg: Color::Rgb(20, 20, 20),
            secondary_bg: Color::Rgb(30, 30, 30),

            primary_text: Color::Rgb(220, 220, 220),
            secondary_text: Color::Rgb(140, 140, 140),
            highlight_text: Color::Rgb(180, 180, 180),
            warning_text: Color::Rgb(160, 160, 160),
            error_text: Color::Rgb(200, 200, 200),

            primary_border: Color::Rgb(80, 80, 80),
            secondary_border: Color::Rgb(60, 60, 60),
            active_border: Color::Rgb(120, 120, 120),

            gauge_primary: Color::Rgb(140, 140, 140),
            gauge_secondary: Color::Rgb(80, 80, 80),
            gauge_background: Color::Rgb(40, 40, 40),

            cpu_low: Color::Rgb(60, 60, 60),
            cpu_medium: Color::Rgb(100, 100, 100),
            cpu_high: Color::Rgb(140, 140, 140),
            cpu_critical: Color::Rgb(180, 180, 180),

            memory_normal: Color::Rgb(80, 80, 80),
            memory_warning: Color::Rgb(120, 120, 120),
            memory_critical: Color::Rgb(160, 160, 160),

            process_normal: Color::Rgb(200, 200, 200),
            process_selected: Color::Rgb(180, 180, 180),
            process_high_cpu: Color::Rgb(160, 160, 160),
            process_info: Color::Rgb(120, 120, 120),

            network_border: Color::Rgb(100, 100, 100),

            animation: Color::Rgb(40, 40, 40),
        }
    }

    pub fn high_contrast() -> Self {
        Self {
            primary_bg: Color::Black,
            secondary_bg: Color::Black,
            
            primary_text: Color::White,
            secondary_text: Color::White,
            highlight_text: Color::Yellow,
            warning_text: Color::Yellow,
            error_text: Color::Red,
            
            primary_border: Color::White,
            secondary_border: Color::White,
            active_border: Color::Yellow,
            
            gauge_primary: Color::White,
            gauge_secondary: Color::White,
            gauge_background: Color::Black,
            
            cpu_low: Color::Green,
            cpu_medium: Color::Yellow,
            cpu_high: Color::Magenta,
            cpu_critical: Color::Red,
            
            memory_normal: Color::White,
            memory_warning: Color::Yellow,
            memory_critical: Color::Red,
            
            process_normal: Color::White,
            process_selected: Color::Yellow,
            process_high_cpu: Color::Red,
            process_info: Color::Cyan,
            
            network_border: Color::White,
            
            animation: Color::Rgb(64, 64, 64),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ThemeType {
    DarkPurple,
    Monotone,
    HighContrast,
}

impl ThemeType {
    pub fn next(&self) -> Self {
        match self {
            ThemeType::DarkPurple => ThemeType::Monotone,
            ThemeType::Monotone => ThemeType::HighContrast,
            ThemeType::HighContrast => ThemeType::DarkPurple,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ThemeType::DarkPurple => "Dark Purple (Default)",
            ThemeType::Monotone => "Monotone",
            ThemeType::HighContrast => "High Contrast",
        }
    }
}

pub struct ThemeManager{
    current_theme_type: ThemeType,
    current_theme: Theme,
}

impl ThemeManager {
    pub fn new() -> Self {
        Self {
            current_theme_type: ThemeType::DarkPurple,
            current_theme: Theme::dark_purple(),
        }
    }

    pub fn switch_theme(&mut self) {
        self.current_theme_type = self.current_theme_type.next();
        self.current_theme = match self.current_theme_type {
            ThemeType::DarkPurple => Theme::dark_purple(),
            ThemeType::Monotone => Theme::monotone(),
            ThemeType::HighContrast => Theme::high_contrast(),
        };
    }

    pub fn current_theme(&self) -> &Theme {
        &self.current_theme
    }

    pub fn current_theme_name(&self) -> &'static str {
        self.current_theme_type.as_str()
    }
}