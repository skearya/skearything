use chrono::Local;

pub fn get_timestamp() -> String {
    // month-day-year, month and day are non-padded
    Local::now().format("%-m-%-d-%Y").to_string()
}

pub fn format_from_seconds(seconds: f32) -> String {
    if seconds > 3600.0 {
        let hours = (seconds / 3600.0).floor();
        let minutes = (seconds % 3600.0) / 60.0;

        return format!("{} and {}", unit("hour", hours), unit("minute", minutes));
    }

    if seconds > 60.0 {
        let minutes = (seconds / 60.0).floor();
        let seconds = seconds % 60.0;

        return format!(
            "{} and {}",
            unit("minute", minutes),
            unit("second", seconds)
        );
    }

    unit("second", seconds)
}

fn unit(unit: &str, num: f32) -> String {
    let extension = if num != 1.0 { "s" } else { "" };

    if num.fract() == 0.0 {
        format!("{} {}{extension}", num, unit)
    } else {
        format!("{:.1} {}{extension}", num, unit)
    }
}
