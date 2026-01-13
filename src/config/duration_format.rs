use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Clone, PartialEq, Eq, Hash, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DurationFormat {
    /// example: "5m 43s"
    Human,
    /// example: "5:43"
    #[default]
    Classic,
    /// example: "%m:%s" or "%M:%S"
    Custom(String),
}

impl DurationFormat {
    pub fn format(&self, total_seconds: u64, separator: Option<&str>) -> String {
        let days = total_seconds / 86400;
        let hours = (total_seconds / 3600) % 24;
        let minutes = (total_seconds / 60) % 60;
        let seconds = total_seconds % 60;

        let human_formatted = {
            if total_seconds == 0 {
                "0s".to_string()
            } else {
                let sep = separator.unwrap_or(" ");
                if days > 0 {
                    format!("{days}d{sep}{hours}h{sep}{minutes}m{sep}{seconds}s")
                } else if hours > 0 {
                    format!("{hours}h{sep}{minutes}m{sep}{seconds}s")
                } else {
                    format!("{minutes}m{sep}{seconds}s")
                }
            }
        };

        match self {
            Self::Custom(template) => template
                .replace("%d", &days.to_string())
                .replace("%D", &format!("{days:02}"))
                .replace("%h", &hours.to_string())
                .replace("%H", &format!("{hours:02}"))
                .replace("%m", &minutes.to_string())
                .replace("%M", &format!("{minutes:02}"))
                .replace("%s", &seconds.to_string())
                .replace("%S", &format!("{seconds:02}")),

            Self::Human => human_formatted,
            _ if separator.is_some() => human_formatted,

            Self::Classic => {
                let total_minutes = total_seconds / 60;
                if days > 0 {
                    format!("{days}d {hours:0>2}:{minutes:0>2}:{seconds:0>2}")
                } else if hours > 0 {
                    format!("{hours}:{minutes:0>2}:{seconds:0>2}")
                } else {
                    format!("{total_minutes}:{seconds:0>2}")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_human_format() {
        let format = DurationFormat::Human;
        assert_eq!(format.format(45, None), "0m 45s");
        assert_eq!(format.format(85, None), "1m 25s");
        assert_eq!(format.format(3665, None), "1h 1m 5s");

        assert_eq!(format.format(95, Some(", ")), "1m, 35s");
    }

    #[test]
    fn test_classic_format() {
        let format = DurationFormat::Classic;
        assert_eq!(format.format(45, None), "0:45");
        assert_eq!(format.format(85, None), "1:25");
        assert_eq!(format.format(3665, None), "1:01:05");
    }

    #[test]
    fn test_custom_format_case_sensitivity() {
        let format = DurationFormat::Custom("%M:%S".to_string());
        assert_eq!(format.format(85, None), "01:25");

        let mixed = DurationFormat::Custom("%h hours, %M mins".to_string());
        assert_eq!(mixed.format(3665, None), "1 hours, 01 mins");
    }

    #[test]
    fn test_default_variant() {
        assert_eq!(DurationFormat::default(), DurationFormat::Classic);
    }

    #[test]
    fn test_ron_config_parsing() {
        let ron_human = "human";
        let de_human: DurationFormat = ron::from_str(ron_human).unwrap();
        assert_eq!(de_human, DurationFormat::Human);

        let ron_custom = "custom(\"%M:%S\")";
        let de_custom: DurationFormat = ron::from_str(ron_custom).unwrap();
        if let DurationFormat::Custom(s) = de_custom {
            assert_eq!(s, "%M:%S");
        }
    }
}
