use std::{io::BufRead, str::FromStr, time::Duration};

use anyhow::Result;
use serde::Serialize;

use super::{LrcOffset, parse_length};

/// Result of parsing a single tag from an LRC line.
#[derive(Debug, Clone)]
enum TagParseResult {
    /// timestamp tag (e.g.: [00:12.34]) with the timestamp content
    Timestamp(String),
    /// metadata tag (e.g.: [ti:Song Title]) with key and value
    Metadata(String, String),
    /// invalid or unrecognized tag
    Invalid,
}

/// Parse a single tag from a line starting with '['.
/// Returns the tag content and the number of characters consumed.
fn parse_next_tag(line: &str) -> Option<(TagParseResult, usize)> {
    if !line.starts_with('[') {
        return None;
    }

    let mut bracket_count = 0;
    let mut close_pos = None;

    for (i, c) in line[1..].char_indices() {
        match c {
            '[' => bracket_count += 1,
            ']' => {
                if bracket_count == 0 {
                    close_pos = Some(i);
                    break;
                }
                bracket_count -= 1;
            }
            _ => {}
        }
    }

    let close_pos = close_pos?;
    let tag_content = &line[1..=close_pos];
    let chars_consumed = close_pos + 2;

    let tag_result = if is_timestamp_tag(tag_content) {
        TagParseResult::Timestamp(tag_content.to_string())
    } else if let Some((key, value)) = tag_content.split_once(':') {
        TagParseResult::Metadata(key.trim().to_string(), value.trim().to_string())
    } else {
        TagParseResult::Invalid
    };

    Some((tag_result, chars_consumed))
}

/// Checks if a tag content represents a timestamp (starts with digit and
/// contains ':').
fn is_timestamp_tag(tag_content: &str) -> bool {
    tag_content.chars().next().is_some_and(|c| c.is_numeric()) && tag_content.contains(':')
}

/// Parse a timestamp string into a Duration.
fn parse_timestamp(timestamp: &str, offset: Option<i64>) -> Option<Duration> {
    let (minutes, time_rest) = timestamp.split_once(':')?;
    let (seconds, fractions_of_second) =
        time_rest.split_once('.').or_else(|| time_rest.split_once(':'))?;

    // fractions of second can be up to 3 digits, truncate if longer
    let fractions_of_second = &fractions_of_second[..3.min(fractions_of_second.len())];

    let (minutes, seconds, fractions) = (
        minutes.parse::<u64>().ok()?,
        seconds.parse::<u64>().ok()?,
        fractions_of_second.parse::<u64>().ok()?,
    );

    let mut millis = 0;
    millis += minutes * 60 * 1000;
    millis += seconds * 1000;
    millis += fractions * (10u64.pow(3 - u32::try_from(fractions_of_second.len()).unwrap_or(0)));

    millis = match offset {
        Some(offset) if offset > 0 => millis.saturating_sub(offset.unsigned_abs()),
        Some(offset) if offset < 0 => millis.saturating_add(offset.unsigned_abs()),
        _ => millis,
    };

    Some(Duration::from_millis(millis))
}

/// A single line of LRC lyrics with its timestamp.
#[derive(Debug, Eq, PartialEq)]
pub struct LrcLine {
    /// The timestamp when this line should be displayed
    time: Duration,
    /// The lyrics content for this line
    pub content: String,
}

impl LrcLine {
    pub fn time(&self, offset: LrcOffset) -> Duration {
        if offset.negative {
            self.time.saturating_add(offset.value)
        } else {
            self.time.saturating_sub(offset.value)
        }
    }
}

/// Parsed LRC file containing metadata and timed lyrics lines.
#[derive(Debug, Eq, PartialEq)]
pub struct Lrc {
    /// The timed lyrics lines, sorted by timestamp
    pub lines: Vec<LrcLine>,
    /// Song title (from [ti:] tag)
    pub title: Option<String>,
    /// Artist name (from [ar:] tag)
    pub artist: Option<String>,
    /// Album name (from [al:] tag)
    pub album: Option<String>,
    /// Author/lyricist name (from [au:] tag)
    pub author: Option<String>,
    /// Song length (from [length:] tag)
    pub length: Option<Duration>,
}

/// Efficiently parse only metadata from LRC content, stopping at the first
/// timestamp. and returning the line index where lyrics start.
pub fn parse_metadata_only(content: &str) -> (LrcMetadata, usize) {
    let mut metadata = LrcMetadata::default();

    for (line_idx, line) in content.lines().enumerate() {
        let line_content = line.trim();
        if line_content.is_empty() || line_content.starts_with('#') {
            continue;
        }

        if !line_content.starts_with('[') {
            continue;
        }

        let mut remaining = line_content;
        let mut found_timestamp = false;

        while let Some((tag_result, chars_consumed)) = parse_next_tag(remaining) {
            match tag_result {
                TagParseResult::Timestamp(_) => {
                    found_timestamp = true;
                    break; // Stop parsing once we hit the first timestamp
                }
                TagParseResult::Metadata(key, value) => match key.as_str() {
                    "ti" => metadata.title = Some(value),
                    "ar" => metadata.artist = Some(value),
                    "al" => metadata.album = Some(value),
                    "au" => metadata.author = Some(value),
                    "length" => {
                        if let Ok(parsed_length) = parse_length(&value) {
                            metadata.length = Some(parsed_length);
                        }
                    }
                    "offset" => {
                        if let Ok(parsed_offset) = value.parse::<i64>() {
                            metadata.offset = Some(parsed_offset);
                        }
                    }
                    _ => {}
                },
                TagParseResult::Invalid => {
                    // Skip invalid tags and continue to next potential tag
                }
            }

            remaining = &remaining[chars_consumed..];
            if !remaining.starts_with('[') {
                break; // No more tags on this line
            }
        }

        if found_timestamp {
            return (metadata, line_idx);
        }
    }

    (metadata, content.lines().count()) // No timestamps found, return end of file
}

/// Metadata extracted from LRC file header tags.
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct LrcMetadata {
    /// Song title (from [ti:] tag)
    pub title: Option<String>,
    /// Artist name (from [ar:] tag)
    pub artist: Option<String>,
    /// Album name (from [al:] tag)
    pub album: Option<String>,
    /// Author/lyricist name (from [au:] tag)
    pub author: Option<String>,
    /// Song length (from [length:] tag)
    pub length: Option<Duration>,
    /// Timing offset in milliseconds (from [offset:] tag)
    pub offset: Option<i64>,
}

impl LrcMetadata {
    pub(super) fn read(mut read: impl BufRead) -> Result<Option<Self>> {
        let mut content = String::new();
        let mut line = String::new();

        loop {
            if read.read_line(&mut line)? == 0 {
                break; // EOF
            }
            // if this line has a timestamp, stop reading
            // We are looking for lines that start with [ and have a timestamp in them
            // reading all the way to the end of the file is not necessary
            let trimmed = line.trim();
            if !trimmed.is_empty()
                && trimmed.starts_with('[')
                && let Some(bracket_end) = trimmed.find(']')
            {
                let tag_content = &trimmed[1..bracket_end];
                if tag_content.chars().next().is_some_and(|c| c.is_numeric())
                    && tag_content.contains(':')
                {
                    // timestamp found, add this line and stop
                    content.push_str(&line);
                    break;
                }
            }
            content.push_str(&line);
            line.clear();
        }

        let (metadata, _) = parse_metadata_only(&content);
        Ok(Some(metadata))
    }
}

impl FromStr for Lrc {
    type Err = anyhow::Error;

    /// Parse a complete LRC file from string content.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (metadata, lyrics_start_line) = parse_metadata_only(s);
        let offset = metadata.offset;

        // preallocate the Vec with an estimated capacity
        // This avoids multiple reallocations during parsing
        let remaining_lines = s.lines().count().saturating_sub(lyrics_start_line);
        let estimated_capacity = remaining_lines * 2;

        let mut result = Self {
            lines: Vec::with_capacity(estimated_capacity),
            title: metadata.title,
            artist: metadata.artist,
            album: metadata.album,
            author: metadata.author,
            length: metadata.length,
        };

        // Process only lines starting from where lyrics begin (skip already-parsed
        // metadata) since we dont want to parse metadata again
        for line in s.lines().skip(lyrics_start_line) {
            let line_content = line.trim();
            if line_content.is_empty() || line_content.starts_with('#') {
                continue;
            }

            if !line_content.starts_with('[') {
                continue;
            }

            let mut timestamps = Vec::new();
            let mut remaining = line_content;
            let mut lyrics_start_pos = 0;

            while let Some((tag_result, chars_consumed)) = parse_next_tag(remaining) {
                match tag_result {
                    TagParseResult::Timestamp(timestamp) => {
                        timestamps.push(timestamp);
                        lyrics_start_pos += chars_consumed;
                        remaining = &remaining[chars_consumed..];

                        if !remaining.starts_with('[') {
                            break;
                        }
                    }
                    TagParseResult::Metadata(_, _) | TagParseResult::Invalid => {
                        // Found a non-timestamp tag, stop here and treat everything
                        // from this position as lyrics content. This is the conservative
                        // approach that handles [00:10.00][Intro] Welcome correctly.
                        break;
                    }
                }
            }

            let lyrics_text = if lyrics_start_pos < line_content.len() {
                &line_content[lyrics_start_pos..]
            } else {
                remaining
            }
            .trim();

            if timestamps.is_empty() {
                continue;
            }

            for timestamp_content in timestamps {
                if let Some(time) = parse_timestamp(&timestamp_content, offset) {
                    result.lines.push(LrcLine { time, content: lyrics_text.to_owned() });
                }
                // if parsing fails, gracefully skip this timestamp
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::time::Duration;

    use super::parse_metadata_only;
    use crate::shared::lrc::{Lrc, lyrics::LrcLine};

    #[test]
    fn lrc() {
        let input = r"[ti: asdf ]
[ar:123]
[al:333]
[au:444]
[length: 2:23]
[offset: +0]

[00:01.86]line with dot before hundredths
[00:04.73]line with colon before hundredths
[00:11.24]
[11:16.91]line with long time";

        let result: Lrc = input.parse().unwrap();

        assert_eq!(result, Lrc {
            title: Some("asdf".to_string()),
            artist: Some("123".to_string()),
            album: Some("333".to_string()),
            author: Some("444".to_string()),
            length: Some(Duration::from_secs(143)),
            lines: vec![
                LrcLine {
                    time: Duration::from_millis(1860),
                    content: "line with dot before hundredths".to_string()
                },
                LrcLine {
                    time: Duration::from_millis(4730),
                    content: "line with colon before hundredths".to_string()
                },
                LrcLine { time: Duration::from_millis(11240), content: String::new() },
                LrcLine {
                    time: Duration::from_millis(676_910),
                    content: "line with long time".to_string()
                },
            ],
        });
    }

    #[test]
    fn lrc_offset_earlier() {
        let input = r"
[offset: +1000]

[00:01.86]line1
[00:04.73]line2
";

        let result: Lrc = input.parse().unwrap();

        assert_eq!(result, Lrc {
            title: None,
            artist: None,
            album: None,
            author: None,
            length: None,
            lines: vec![
                LrcLine { time: Duration::from_millis(860), content: "line1".to_string() },
                LrcLine { time: Duration::from_millis(3730), content: "line2".to_string() },
            ],
        });
    }

    #[test]
    fn lrc_offset_later() {
        let input = r"
[offset: -1000]

[00:01.86]line1
[00:04.73]line2
";

        let result: Lrc = input.parse().unwrap();

        assert_eq!(result, Lrc {
            title: None,
            artist: None,
            album: None,
            author: None,
            length: None,
            lines: vec![
                LrcLine { time: Duration::from_millis(2860), content: "line1".to_string() },
                LrcLine { time: Duration::from_millis(5730), content: "line2".to_string() },
            ],
        });
    }

    #[test]
    fn repeating_lyrics() {
        let input = r"
[00:01.86]line1
[00:04.73][00:05.73][00:06.73]line2
[00:07.86]line3
";

        let result: Lrc = input.parse().unwrap();

        assert_eq!(result, Lrc {
            title: None,
            artist: None,
            album: None,
            author: None,
            length: None,
            lines: vec![
                LrcLine { time: Duration::from_millis(1860), content: "line1".to_string() },
                LrcLine { time: Duration::from_millis(4730), content: "line2".to_string() },
                LrcLine { time: Duration::from_millis(5730), content: "line2".to_string() },
                LrcLine { time: Duration::from_millis(6730), content: "line2".to_string() },
                LrcLine { time: Duration::from_millis(7860), content: "line3".to_string() },
            ],
        });
    }

    #[test]
    fn lyrics_different_fractions_of_second() {
        let input = r"
[00:00.8]line1
[00:10.73]line2
[00:20.563]line3
[00:30.2853]line4
";

        let result: Lrc = input.parse().unwrap();

        assert_eq!(result, Lrc {
            title: None,
            artist: None,
            album: None,
            author: None,
            length: None,
            lines: vec![
                LrcLine { time: Duration::from_millis(800), content: "line1".to_string() },
                LrcLine { time: Duration::from_millis(10730), content: "line2".to_string() },
                LrcLine { time: Duration::from_millis(20563), content: "line3".to_string() },
                LrcLine { time: Duration::from_millis(30285), content: "line4".to_string() },
            ]
        });
    }

    #[test]
    fn brackets_in_lyrics_text() {
        let input = r"
[ti: Song Name [Explicit]]
[00:09.00]
[00:10.00] [Drum Solo]
[00:11.00]Some text [with brackets] in lyrics
";

        let result: Lrc = input.parse().unwrap();

        assert_eq!(result, Lrc {
            title: Some("Song Name [Explicit]".to_string()),
            artist: None,
            album: None,
            author: None,
            length: None,
            lines: vec![
                LrcLine { time: Duration::from_millis(9000), content: String::new() },
                LrcLine { time: Duration::from_millis(10000), content: "[Drum Solo]".to_string() },
                LrcLine {
                    time: Duration::from_millis(11000),
                    content: "Some text [with brackets] in lyrics".to_string()
                },
            ]
        });
    }

    #[test]
    fn edge_case_empty_tags() {
        let input = r"
[ti:]
[ar:]
[al:]
[00:10.00]lyrics after empty tags
";

        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.title, Some(String::new()));
        assert_eq!(result.artist, Some(String::new()));
        assert_eq!(result.album, Some(String::new()));
        assert_eq!(result.lines.len(), 1);
    }

    #[test]
    fn edge_case_whitespace_handling() {
        let input = r"
[ti:  Title with spaces  ]
[ar:	Artist with tabs	]
[00:10.00]   lyrics with leading/trailing spaces   
";

        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.title, Some("Title with spaces".to_string()));
        assert_eq!(result.artist, Some("Artist with tabs".to_string()));
        assert_eq!(result.lines[0].content, "lyrics with leading/trailing spaces");
    }

    #[test]
    fn edge_case_mixed_timestamp_formats() {
        let input = r"
[00:01.5]single digit fraction
[00:02.75]two digit fraction
[00:03.123]three digit fraction
[00:04.1234]four digit fraction (should truncate)
[1:05.50]single digit minute
[12:06.50]two digit minute
";

        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.lines.len(), 6);
        assert_eq!(result.lines[0].time, Duration::from_millis(1500));
        assert_eq!(result.lines[1].time, Duration::from_millis(2750));
        assert_eq!(result.lines[2].time, Duration::from_millis(3123));
        assert_eq!(result.lines[3].time, Duration::from_millis(4123)); // truncated
        assert_eq!(result.lines[4].time, Duration::from_millis(65500));
        assert_eq!(result.lines[5].time, Duration::from_millis(726_500));
    }

    #[test]
    fn edge_case_colon_separator_in_timestamp() {
        let input = r"
[00:01:50]using colon instead of dot
[00:02:123]colon with three digit fraction
";

        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.lines.len(), 2);
        assert_eq!(result.lines[0].time, Duration::from_millis(1500));
        assert_eq!(result.lines[1].time, Duration::from_millis(2123));
    }

    #[test]
    fn edge_case_complex_brackets_in_lyrics() {
        let input = r"
[ti:Song [Feat. Artist] (Remix)]
[00:10.00][Intro] Welcome to the [Show]
[00:20.00]Lyrics with [multiple] [brackets] here
[00:30.00]Even [nested [brackets] work] fine
[00:40.00]And [some] text [with] [many] [brackets]
";

        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.title, Some("Song [Feat. Artist] (Remix)".to_string()));
        assert_eq!(result.lines[0].content, "[Intro] Welcome to the [Show]");
        assert_eq!(result.lines[1].content, "Lyrics with [multiple] [brackets] here");
        assert_eq!(result.lines[2].content, "Even [nested [brackets] work] fine");
        assert_eq!(result.lines[3].content, "And [some] text [with] [many] [brackets]");
    }

    #[test]
    fn edge_case_offset_variations() {
        let input = r"
[offset:+500]
[00:01.00]offset positive no space
";

        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.lines[0].time, Duration::from_millis(500));

        let input2 = r"
[offset: -250]
[00:01.00]offset negative with space
";

        let result2: Lrc = input2.parse().unwrap();
        assert_eq!(result2.lines[0].time, Duration::from_millis(1250));
    }

    #[test]
    fn edge_case_unknown_metadata_tags() {
        let input = r"
[ti:Title]
[ar:Artist] 
[custom:unknown tag]
[version:1.0]
[tool:rmpc]
[00:10.00]lyrics
";

        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.title, Some("Title".to_string()));
        assert_eq!(result.artist, Some("Artist".to_string()));
        assert_eq!(result.lines.len(), 1);
        // Unknown tags should be ignored
    }

    #[test]
    fn edge_case_multiple_consecutive_empty_lines() {
        let input = r"
[ti:Title]

[00:10.00]first line

[00:20.00]

[00:30.00]third line
";

        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.lines.len(), 3);
        assert_eq!(result.lines[0].content, "first line");
        assert_eq!(result.lines[1].content, "");
        assert_eq!(result.lines[2].content, "third line");
    }

    #[test]
    fn edge_case_comments_and_invalid_lines() {
        let input = r"
# This is a comment
[ti:Title]
# Another comment
[ar:Artist]
invalid line without brackets
[00:10.00]valid lyrics
# End comment
";

        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.title, Some("Title".to_string()));
        assert_eq!(result.artist, Some("Artist".to_string()));
        assert_eq!(result.lines.len(), 1);
        assert_eq!(result.lines[0].content, "valid lyrics");
    }

    #[test]
    fn edge_case_length_parsing_variations() {
        let input = r"
[length:3:45]
[00:10.00]lyrics
";

        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.length, Some(Duration::from_secs(225)));

        let input2 = r"
[length: 2:30 ]
[00:10.00]lyrics
";

        let result2: Lrc = input2.parse().unwrap();
        assert_eq!(result2.length, Some(Duration::from_secs(150)));
    }

    #[test]
    fn edge_case_very_long_timestamps() {
        let input = r"
[99:59.99]very long timestamp
[123:45.67]even longer
";

        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.lines.len(), 2);
        assert_eq!(result.lines[0].time, Duration::from_millis(5_999_990));
        assert_eq!(result.lines[1].time, Duration::from_millis(7_425_670));
    }

    #[test]
    fn edge_case_zero_padding_timestamps() {
        let input = r"
[00:00.00]start
[00:01.01]one second
[01:00.00]one minute
";

        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.lines.len(), 3);
        assert_eq!(result.lines[0].time, Duration::from_millis(0));
        assert_eq!(result.lines[1].time, Duration::from_millis(1010));
        assert_eq!(result.lines[2].time, Duration::from_millis(60000));
    }

    #[test]
    fn edge_case_unicode_and_special_characters() {
        let input = r"
[ti:Café Münü 🎵]
[ar:Artíst Namé]
[00:10.00]Lyrics with émojis 🎶 and accénts
[00:20.00]More unicode: 你好 世界
[00:30.00]Special chars: @#$%^&*()
";

        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.title, Some("Café Münü 🎵".to_string()));
        assert_eq!(result.artist, Some("Artíst Namé".to_string()));
        assert_eq!(result.lines[0].content, "Lyrics with émojis 🎶 and accénts");
        assert_eq!(result.lines[1].content, "More unicode: 你好 世界");
        assert_eq!(result.lines[2].content, "Special chars: @#$%^&*()");
    }

    #[test]
    fn edge_case_malformed_brackets() {
        let input = r"
[ti:Title]
[unclosed bracket
[00:10.00]valid line
]orphaned closing bracket
[00:20.00]another valid line
";

        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.title, Some("Title".to_string()));
        assert_eq!(result.lines.len(), 2);
        // Malformed lines should be ignored
    }

    #[test]
    fn stress_test_many_consecutive_timestamps() {
        let input = r"
[00:01.00][00:02.00][00:03.00][00:04.00][00:05.00]repeated chorus
[00:10.00][00:11.00][00:12.00]another repeated part
";

        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.lines.len(), 8);
        // First 5 lines should all have "repeated chorus"
        for i in 0..5 {
            assert_eq!(result.lines[i].content, "repeated chorus");
        }
        // Next 3 lines should have "another repeated part"
        for i in 5..8 {
            assert_eq!(result.lines[i].content, "another repeated part");
        }
    }

    #[test]
    fn error_handling_invalid_timestamp_format() {
        let input = r"
[invalid:timestamp]lyrics
";
        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.lines.len(), 0); // Invalid timestamp should be ignored
    }

    #[test]
    fn error_handling_invalid_minutes() {
        let input = r"
[ti:Title]
[abc:30.00]invalid minutes
[00:10.00]valid line
";
        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.lines.len(), 1); // Only valid line should be parsed
    }

    #[test]
    fn error_handling_invalid_seconds() {
        let input = r"
[ti:Title]
[00:abc.00]invalid seconds
[00:10.00]valid line
";
        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.lines.len(), 1); // Only valid line should be parsed
    }

    #[test]
    fn error_handling_invalid_fraction() {
        let input = r"
[ti:Title]
[00:30.abc]invalid fraction
[00:10.00]valid line
";
        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.lines.len(), 1); // Only valid line should be parsed
    }

    #[test]
    fn error_handling_invalid_offset() {
        let input = r"
[offset:invalid]
[00:10.00]should work with invalid offset ignored
";
        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.lines.len(), 1);
        assert_eq!(result.lines[0].time, Duration::from_millis(10000)); // No offset applied
    }

    #[test]
    fn error_handling_invalid_length() {
        let input = r"
[length:invalid]
[00:10.00]should work with invalid length ignored
";
        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.lines.len(), 1);
        assert_eq!(result.length, None); // Length should be None due to invalid format
    }

    #[test]
    fn robustness_test_empty_file() {
        let input = "";
        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.lines.len(), 0);
        assert_eq!(result.title, None);
        assert_eq!(result.artist, None);
    }

    #[test]
    fn robustness_test_whitespace_only() {
        let input = "   \n\t\n  \n";
        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.lines.len(), 0);
    }

    #[test]
    fn robustness_test_comments_only() {
        let input = r"
# Comment 1
# Comment 2
# Comment 3
";
        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.lines.len(), 0);
    }

    #[test]
    fn boundary_test_minimum_valid_lrc() {
        let input = r"[00:00.00]";
        let result: Lrc = input.parse().unwrap();
        assert_eq!(result.lines.len(), 1);
        assert_eq!(result.lines[0].time, Duration::from_millis(0));
        assert_eq!(result.lines[0].content, "");
    }

    #[test]
    fn parse_metadata_only_basic() {
        let input = r"
[ti:Test Title]
[ar:Test Artist]
[al:Test Album]
[au:Test Author]
[length:3:45]
[offset:+1000]
[00:10.00]This is a lyrics line
[00:20.00]This is another lyrics line
";

        let (metadata, _) = parse_metadata_only(input);

        assert_eq!(metadata.title, Some("Test Title".to_string()));
        assert_eq!(metadata.artist, Some("Test Artist".to_string()));
        assert_eq!(metadata.album, Some("Test Album".to_string()));
        assert_eq!(metadata.author, Some("Test Author".to_string()));
        assert_eq!(metadata.length, Some(Duration::from_secs(225)));
    }

    #[test]
    fn parse_metadata_only_stops_at_timestamp() {
        let input = r"
[ti:Test Title]
[ar:Test Artist]
[00:10.00]This is a lyrics line
[al:Test Album]
[au:Test Author]
[length:3:45]
";

        let (metadata, start_line) = parse_metadata_only(input);

        assert_eq!(metadata.title, Some("Test Title".to_string()));
        assert_eq!(metadata.artist, Some("Test Artist".to_string()));
        assert_eq!(metadata.album, None);
        assert_eq!(metadata.author, None);
        assert_eq!(metadata.length, None);
        assert_eq!(start_line, 3); // Should stop at line with timestamp
    }

    #[test]
    fn parse_metadata_only_with_brackets_in_metadata() {
        let input = r"
[ti:Song [Explicit] (Remix)]
[ar:Artist [Feat. Someone]]
[al:Album [Deluxe Edition]]
[00:10.00]lyrics
";

        let (metadata, _) = parse_metadata_only(input);

        assert_eq!(metadata.title, Some("Song [Explicit] (Remix)".to_string()));
        assert_eq!(metadata.artist, Some("Artist [Feat. Someone]".to_string()));
        assert_eq!(metadata.album, Some("Album [Deluxe Edition]".to_string()));
    }

    #[test]
    fn parse_metadata_only_graceful_error_handling() {
        let input = r"
[ti:Valid Title]
[ar:Valid Artist]
[invalid_tag_without_colon]
[invalid:length:value]
[length:invalid_format]
[00:10.00]lyrics
";

        let (metadata, _) = parse_metadata_only(input);

        assert_eq!(metadata.title, Some("Valid Title".to_string()));
        assert_eq!(metadata.artist, Some("Valid Artist".to_string()));
        assert_eq!(metadata.length, None); // invalid length should be ignored
    }

    #[test]
    fn parse_metadata_only_empty_and_whitespace() {
        let input = r"
[ti:  Title with spaces  ]
[ar:	Artist with tabs	]
[al:]
[au:   ]
[00:10.00]lyrics
";

        let (metadata, _) = parse_metadata_only(input);

        assert_eq!(metadata.title, Some("Title with spaces".to_string()));
        assert_eq!(metadata.artist, Some("Artist with tabs".to_string()));
        assert_eq!(metadata.album, Some(String::new()));
        assert_eq!(metadata.author, Some(String::new()));
    }

    #[test]
    fn edge_case_malformed_tags_between_timestamps() {
        let input1 = r"
[00:10.00][str:rst][00:12.00]repeated with mangled tag in the middle
";
        let result1: Lrc = input1.parse().unwrap();
        assert_eq!(result1.lines.len(), 1);
        assert_eq!(result1.lines[0].time, Duration::from_millis(10000));
        assert_eq!(
            result1.lines[0].content,
            "[str:rst][00:12.00]repeated with mangled tag in the middle"
        );

        let input2 = r"
[00:10.00][str][00:12.00]repeated with mangled tag in the middle
";
        let result2: Lrc = input2.parse().unwrap();
        assert_eq!(result2.lines.len(), 1);
        assert_eq!(result2.lines[0].time, Duration::from_millis(10000));
        assert_eq!(
            result2.lines[0].content,
            "[str][00:12.00]repeated with mangled tag in the middle"
        );

        let input3 = r"
[00:10.00][00:12.00][00:14.00]properly repeated line
";
        let result3: Lrc = input3.parse().unwrap();
        assert_eq!(result3.lines.len(), 3);
        assert_eq!(result3.lines[0].time, Duration::from_millis(10000));
        assert_eq!(result3.lines[0].content, "properly repeated line");
        assert_eq!(result3.lines[1].time, Duration::from_millis(12000));
        assert_eq!(result3.lines[1].content, "properly repeated line");
        assert_eq!(result3.lines[2].time, Duration::from_millis(14000));
        assert_eq!(result3.lines[2].content, "properly repeated line");
    }
}
