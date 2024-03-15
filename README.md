## Header Config
Header is at the top of the window and is used to display the current song and some other information. Header can be configured to display different information in six different places:
* top_left
* top_center
* top_right
* bottom_left
* bottom_center
* bottom_right

Each of these places can contain multiple properties. The widgets are displayed in the order they are specified in the config. Possible properties are:
* Song([SongProperty](#song-property))
* Status([StatusProperty](#status-property))
* Widget([WidgetProperty](#widget-property))
* Text(value: String, style: Option<[StyleDef](#styledef)>)


### Example Header Config
This is the default header config. It displays the current song title in the top center, artist and album in the bottom center, state in the top left, elapsed time and bitrate in the bottom left, volume in the top right and the repeat, single, consume and random states in the bottom right.
```rust,ignore
header: (
    top_center: [Song(Title(style: (fg: None, bg: None, modifiers: "Bold"), default: "No Song"))],
    bottom_center: [Song(Artist(style: (fg: "yellow", bg: None, modifiers: "Bold"), default: "Unknown")), Text(value: " - ", style: None), Song(Album(style: (fg: "blue", bg: None, modifiers: "Bold"), default: "Unknown Album"))],
    top_left: [Text(value: "[", style: (fg: "yellow", bg: None, modifiers: "Bold")), Status(State(style: (fg: "yellow", bg: None, modifiers: "Bold"))), Text(value: "]", style: (fg: "yellow", bg: None, modifiers: "Bold"))],
    bottom_left: [Status(Elapsed(style: None)), Text(value: "/", style: None), Text(value: " (", style: None), Status(Bitrate(style: None, default: "-")), Text(value: " kbps)", style: None)],
    top_right: [Widget(Volume(style: (fg: "blue", bg: None, modifiers: None)))],
    bottom_right: [Widget(States(active_style: (fg: "white", bg: None, modifiers: "Bold"), inactive_style: (fg: "dark_gray", bg: None, modifiers: None), separator_style: (fg: "white", bg: None, modifiers: None)))],
),
```

## Song Table Format
The columns in queue screen and their sizes can be configured.
The configuration is a list of column definitions in the following format:

(prop: [SongProperty](#song-property), label: Option<[Label](#label)>, width_percent: u16, alignment: Option<[Alignment](#alignment)>).


### Label
Label is an optional string that is displayed as the column header. If it is not present the prop name is used.

### Width Percent
Width percent is the width of the column in percent of the total width of the table. All widths have to add up to 100.

### Alignment
Alignment is an optional field that specifies the alignment of the text in the column. Possible values are:
* Left
* Right
* Center


### Example Song Table Format
This is the default song table format. It shows four columns: Artist, Title, Album and Duration.
```rust,ignore
song_table_format: [
    (prop: Artist(style: None, default: "Unknown"),                                          label: None, width_percent: 20, alignment: None),
    (prop: Title(style: None, default: "Unknown"),                                           label: None, width_percent: 35, alignment: None),
    (prop: Album(style: (fg: "white", bg: None, modifiers: None), default: "Unknown Album"), label: None, width_percent: 30, alignment: None),
    (prop: Duration(style: None, default: "-"),                                              label: None, width_percent: 15, alignment: Right)
],
```

## Song Property
Can be any of the following:
* Filename(style: Option<[StyleDef](#styledef)>)
* Title(style: Option<[StyleDef](#styledef)>,    default: String)
* Artist(style: Option<[StyleDef](#styledef)>,   default: String)
* Album(style: Option<[StyleDef](#styledef)>,    default: String)
* Duration(style: Option<[StyleDef](#styledef)>, default: String)
* Other(name: String, style: Option<[StyleDef](#styledef)>, default: String)

The Prop `Other` is used to display any other tag that might be present on the song. For example "Genre" or "Year".

Values that are not guaranteed to be present on the song have to have default values specified.

### Example Song Property
Shows the album tag of the song with a white foreground color, default background color and the default value "Unknown Album"
```rust,ignore
Album(style: (fg: "white", bg: None, modifiers: None), default: "Unknown Album")
```
## Status Property
Can be any of the following:
* Volume(style: Option<[StyleDef](#styledef)>)
* Repeat(style: Option<[StyleDef](#styledef)>)
* Random(style: Option<[StyleDef](#styledef)>)
* Single(style: Option<[StyleDef](#styledef)>)
* Consume(style: Option<[StyleDef](#styledef)>)
* State(style: Option<[StyleDef](#styledef)>)
* Elapsed(style: Option<[StyleDef](#styledef)>)
* Duration(style: Option<[StyleDef](#styledef)>)
* Crossfade(style: Option<[StyleDef](#styledef)>, default: String)
* Bitrate(style: Option<[StyleDef](#styledef)>, default: String)

Values that are not guaranteed to be present on the song have to have default values specified.

## Widget Property
Widget properties are a few predefined "widgets" that can be displayed in the header. They are special because they have additional capabilities and/or styling options.

### States widget
Offers additional styling for active and inactive state. Looks like this: `Repeat / Random / Consume / Single`. Where the active states are highlighted with the active style and the inactive states are highlighted with the inactive style. The '/' is highlighted with the separator style.

States(active_style: Option<[StyleDef](#styledef)>, inactive_style: Option<[StyleDef](#styledef)>, separator_style: Option<[StyleDef](#styledef)>)

### Volume widget
Shows volume with percentage and bars instead of just simple number. Looks like this: `Volume: ▁▂▃▄▅▆▇ 100%`

Format: Volume(style: Option<[StyleDef](#styledef)>)

## StyleDef
A `StyleDef` is a tuple with the following fields:
* fg: Option<[Color](#color-format)> - a foreground color
* bg: Option<[Color](#color-format)> - a background color
* modifiers: Option<[Modifiers](#modifiers)> - text modifiers

### Color format
Colors are specified as string. Supported values are: 
* the 16 terminal colors as text - `"black" | "red" | "green" | "yellow" | "blue" | "magenta" | "cyan" | "gray" | "dark_gray" | "light_red" | "light_green" | "light_yellow" | "light_blue" | "light_magenta" | "light_cyan" | "white"`
* hex value - `"#ff0000"`
* rgb value `"rgb(255, 0, 0)"`
* number of the 256 terminal colors.. - `"196"`

### Modifiers
Possible modifiers for styles are:
* Bold
* Dim
* Italic
* Underlined
* Reversed
* CrossedOut

## Example Config
This is the default config. You can also generate it by running `rmpc --config`

```rust,ignore
#![enable(implicit_some)]
#![enable(unwrap_newtypes)]
#![enable(unwrap_variant_newtypes)]
(
    // MPD address to connect to
    address: "127.0.0.1:6600",
    // Adjust voume by this amount %
    volume_step: 5,
    // How often to update the progress bar in milliseconds
    status_update_interval_ms: 1000,
    keybinds: (
        // Global keybinds are as the name implies, global. On every screen except when modal is active.
        // Possible modifiers are: SHIFT, CONTROL, ALT, SUPER, HYPER, META.
        // To specify multiple modifiers, separate them with a | sign, ie. SHIFT | CONTROL.
        // You can also bind more than one key to the same action by specifying multiple keybinds.
        global: {
            ToggleRepeat:   [(key: Char('z'), modifiers: "")],
            NextTrack:      [(key: Char('>'), modifiers: "")],
            ToggleSingle:   [(key: Char('c'), modifiers: "")],
            ArtistsTab:     [(key: Char('3'), modifiers: "")],
            TogglePause:    [(key: Char('p'), modifiers: "")],
            PlaylistsTab:   [(key: Char('5'), modifiers: "")],
            Stop:           [(key: Char('s'), modifiers: "")],
            DirectoriesTab: [(key: Char('2'), modifiers: "")],
            SeekBack:       [(key: Char('b'), modifiers: "")],
            ToggleRandom:   [(key: Char('x'), modifiers: "")],
            VolumeDown:     [(key: Char(','), modifiers: "")],
            PreviousTrack:  [(key: Char('<'), modifiers: "")],
            SeekForward:    [(key: Char('f'), modifiers: "")],
            VolumeUp:       [(key: Char('.'), modifiers: "")],
            Quit:           [(key: Char('q'), modifiers: "")],
            ToggleConsume:  [(key: Char('v'), modifiers: "")],
            AlbumsTab:      [(key: Char('4'), modifiers: "")],
            QueueTab:       [(key: Char('1'), modifiers: "")],
            NextTab:        [(key: Right,     modifiers: ""), (key: Tab,     modifiers: "")],
            PreviousTab:    [(key: Left,      modifiers: ""), (key: BackTab, modifiers: "SHIFT")],
        },
        navigation: {
            Up:             [(key: Char('k'), modifiers: "")],
            Right:          [(key: Char('l'), modifiers: "")],
            Close:          [(key: Char('c'), modifiers: "CONTROL"), (key: Esc, modifiers: "")],
            Select:         [(key: Char(' '), modifiers: "")],
            Confirm:        [(key: Enter,     modifiers: "")],
            MoveUp:         [(key: Char('K'), modifiers: "SHIFT")],
            MoveDown:       [(key: Char('J'), modifiers: "SHIFT")],
            Top:            [(key: Char('g'), modifiers: "")],
            NextResult:     [(key: Char('n'), modifiers: "CONTROL")],
            Bottom:         [(key: Char('G'), modifiers: "SHIFT")],
            Down:           [(key: Char('j'), modifiers: "")],
            Delete:         [(key: Char('D'), modifiers: "SHIFT")],
            UpHalf:         [(key: Char('u'), modifiers: "CONTROL")],
            FocusInput:     [(key: Char('i'), modifiers: "")],
            EnterSearch:    [(key: Char('/'), modifiers: "")],
            DownHalf:       [(key: Char('d'), modifiers: "CONTROL")],
            PreviousResult: [(key: Char('N'), modifiers: "SHIFT")],
            Left:           [(key: Char('h'), modifiers: "")],
            Rename:         [(key: Char('r'), modifiers: "")],
            Add:            [(key: Char('a'), modifiers: "")],
        },
        albums: {},
        artists: {},
        directories: {},
        playlists: {},
        logs: {
            Clear:          [(key: Char('D'), modifiers: "SHIFT")],
        },
        queue: {
            Save:           [(key: Char('s'), modifiers: "CONTROL")],
            DeleteAll:      [(key: Char('D'), modifiers: "SHIFT")],
            Play:           [(key: Enter,     modifiers: "")],
            AddToPlaylist:  [(key: Char('a'), modifiers: "")],
            Delete:         [(key: Char('d'), modifiers: "")],
        },
    ),
    // Possible modifiers for styles are: Bold, Dim, Italic, Underlined, Reversed, CrossedOut
    // Colors are specified as string.
    // Supported values are: 
    // * the 16 terminal colors
    // * hex value (eg. "#ff0000")
    // * rgb value (eg. "rgb(255, 0, 0)")
    // * number (eg. "196") of the 256 terminal colors..
    ui: (
        album_art_position: Left,
        album_art_width_percent: 40,
        draw_borders: true,
        // Symbols used in the various song browsers
        // Use this symbol to indicate the item is marked
        symbols: (song: "S", dir: "D", marker: "M"),
        progress_bar: (
            // Progress bar at the bottom of the window.
            // First symbol is the elapsed part of the track, second is the thumb, third is the remaining part.
            symbols: ["-", ">", " "],
            track_style: (fg: "#1e2030", bg: None, modifiers: None),
            elapsed_style: (fg: "blue", bg: None, modifiers: None),
            thumb_style: (fg: "blue", bg: "#1e2030", modifiers: None),
        ),
        scrollbar: (
            // Scorllbar symbols. First is the vertical line, second is the thumb, third is the up arrow, fourth is the down arrow.
            symbols: ["│", "█", "▲", "▼"],
            track_style: (fg: None, bg: None, modifiers: None),
            ends_style: (fg: None, bg: None, modifiers: None),
            thumb_style: (fg: "blue", bg: None, modifiers: None),
        ),
        // Ratio of the width of the columns in the song browser.
        browser_column_widths: [20, 38, 42],
        background_color: None,
        header_background_color: None,
        background_color_modal: None,
        show_song_table_header: true,
        active_tab_style: (fg: "black", bg: "blue", modifiers: "Bold"),
        inactive_tab_style: (fg: None,bg: None,modifiers: None),
        borders_style: (fg: "blue", bg: None, modifiers: None),
        current_song_style: (fg: "blue", bg: None, modifiers: "Bold"),
        highlight_style: (fg: "black", bg: "blue", modifiers: "Bold"),
        highlight_border_style: (fg: "blue", bg: None, modifiers: None),
        // Table and header formats are explained in their section
        song_table_format: [
            (prop: Artist(style: None, default: "Unknown"), label: None, width_percent: 20, alignment: None),
            (prop: Title(style: None, default: "Unknown"), label: None, width_percent: 35, alignment: None),
            (prop: Album(style: (fg: "white", bg: None, modifiers: None), default: "Unknown Album"), label: None, width_percent: 30, alignment: None),
            (prop: Duration(style: None, default: "-"), label: None, width_percent: 15, alignment: Right)
        ],
        header: (
            top_center: [Song(Title(style: (fg: None, bg: None, modifiers: "Bold"), default: "No Song"))],
            bottom_center: [Song(Artist(style: (fg: "yellow", bg: None, modifiers: "Bold"), default: "Unknown")), Text(value: " - ", style: None), Song(Album(style: (fg: "blue", bg: None, modifiers: "Bold"), default: "Unknown Album"))],
            top_left: [Text(value: "[", style: (fg: "yellow", bg: None, modifiers: "Bold")), Status(State(style: (fg: "yellow", bg: None, modifiers: "Bold"))), Text(value: "]", style: (fg: "yellow", bg: None, modifiers: "Bold"))],
            bottom_left: [Status(Elapsed(style: None)), Text(value: "/", style: None), Text(value: " (", style: None), Status(Bitrate(style: None, default: "-")), Text(value: " kbps)", style: None)],
            top_right: [Widget(Volume(style: (fg: "blue", bg: None, modifiers: None)))],
            bottom_right: [Widget(States(active_style: (fg: "white", bg: None, modifiers: "Bold"), inactive_style: (fg: "dark_gray", bg: None, modifiers: None), separator_style: (fg: "white", bg: None, modifiers: None)))],
        ),
    ),
)
```
