//! The quest book (#1560; docs/re/questbook.md): `L` and the bar's book button
//! (`UI_TB_BOOK`) show and hide it (`cUI_Diary`, `UI_WND_QUESTBOOK`, `0x6AEAC0`), with
//! `UI_QUESTBOOK_CLOSE01` either way, and its close button and Esc hide it; its show hides the
//! inventory, as the inventory's show hides it. It is laid out on the original's 1024 × 768
//! screen centred along the window's width, as the bar is (`bar`).
//!
//! Its picture of six pieces, its four tabs above it, the selected one raised and a click on one
//! selecting it with `UI_QUESTBOOK_TURN01`, the parchment of its two pages over them, each page's
//! of a set rolled once a game, and its close button. Each show builds the selected tab's book
//! again from the quest log as it stands then (`0x6B1990`), with `TURN01`, keeping the chapter,
//! the list's page and its selected row.
//!
//! The quests' book (`0x6B07E0`): a bookmark at the book's edge for each chapter that holds an
//! entry with a title, lit while it is selected or under the pointer, a click on it selecting its
//! chapter with `TURN01`; the left page the chapter's title and its entries in the order the
//! quests entered the log, an ornament first on each page and before a main quest that follows
//! another kind, each entry's icon by its kind and how it stands, and its title in Carolingia for
//! a main quest, in AntiquaSSK and further in for a side one, in the colour of how it stands,
//! whether it is new, selected or under the pointer; a page of the list ends once its rows reach
//! 334 pixels down, its arrows and its count under it. A click on a row selects it, and on a side
//! quest that runs makes it the one the compass's inner pin follows (`0x49DAB0`). The right page
//! the selected entry's, else the chapter's first: its title, the minimap's ground about its
//! compass's place (`minimap`), and its lines with a blank line between, poured over pages of
//! their own, each with the title, their arrows and their count. Texts are laid out and measured
//! by Bevy, a hidden node each, for the pages' breaks.
//!
//! Not followed yet: the hero's, the information's and the statistics' books (#1565), the
//! bar's button's pressed look, and the place of a compass on a creature taken where the
//! creature stands as the book is built, not as the quest was entered.

use std::collections::HashMap;

use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::text::{FontSize, Justify, LineHeight};
use bevy::ui::ComputedNode;
use opensacred_rules::quests::{Entry, Standing};
use opensacred_rules::spawning::Rng;

use crate::bar::{self, UiImages};
use crate::frame::Control;
use crate::game::{AppState, FONT};
use crate::hud::Pointer;
use crate::inventory::screen_point;
use crate::lines::Texts;
use crate::menu::MENU_FONT;
use crate::sfx;
use crate::tooltip::{Rest, TipRoot, argb, spawn_tip};

/// The book's picture: six pieces of 256 × 256 from (128, 128), three across (`GUI_BOOK_BASE1` to
/// `6`), and the rectangle that takes the pointer (`0x6AF1B0`).
pub(crate) const BASES: [&str; 6] = [
    "gui_book_base1",
    "gui_book_base2",
    "gui_book_base3",
    "gui_book_base4",
    "gui_book_base5",
    "gui_book_base6",
];
const BASE_AT: Vec2 = Vec2::new(128.0, 128.0);
const BASE: f32 = 256.0;
const TAKES: Rect = Rect {
    min: Vec2::new(128.0, 64.0),
    max: Vec2::new(896.0, 640.0),
};
/// The pages' parchment: four images of a set (`GUI_BOOK_SHEET1A` …, the images `0x5A7` on, four
/// a set), at (0, 0) and (0, 256) of each page, 256 wide, and at (256, 0) and (256, 256), 128
/// wide; the left page from x 128, the right from 512 (`0x6AF440`). A page's set is rolled once:
/// one of the first three (`rand() / 32767 · 3`).
const SHEETS_FROM: u32 = 0x5A7;
const SHEET_SETS: u32 = 3;
const SHEET_AT: [Vec2; 4] = [
    Vec2::new(0.0, 0.0),
    Vec2::new(0.0, 256.0),
    Vec2::new(256.0, 0.0),
    Vec2::new(256.0, 256.0),
];
const PAGES_X: [f32; 2] = [128.0, 512.0];
/// The tabs (`0x6AF250`, `0x6AF0E0`): at x 164 + 72 · i, y 96, the selected one at 72; the quests',
/// the information's and the statistics' images, the hero's by its type, and their tooltips (at
/// the tab's top, 32 along).
const TABS: usize = 4;
const TAB_X: f32 = 164.0;
const TAB_STEP: f32 = 72.0;
const TAB_Y: f32 = 96.0;
const TAB_RAISED: f32 = 72.0;
const TAB_SIZE: Vec2 = Vec2::new(64.0, 102.0);
const QUESTS_TAB: u32 = 0x5BD;
const INFO_TAB: u32 = 0x5BC;
const STATS_TAB: u32 = 0x5BF;
const TAB_TIPS: [&str; TABS] = [
    "UI_TOOLTIP_QBOOK_QUESTS",
    "UI_TOOLTIP_QBOOK_HERO",
    "UI_TOOLTIP_QBOOK_INFO",
    "UI_TOOLTIP_QBOOK_STATS",
];
/// The close button (`GUI_main_15`, 34 × 34).
const CLOSE: (u32, Vec2) = (0x167, Vec2::new(861.0, 129.0));
const CLOSE_SIDE: f32 = 34.0;
/// The chapters' bookmarks (`0x6B3F90`): the first five down the left edge from (86, 184), the
/// last two down the right from (866, 184), 80 apart, each moved left by a roll of 0 to 3
/// (`rand() / 32767 · 4`); their images and lit images (`0x9E075C`).
const CHAPTERS: usize = 7;
const LEFT_MARKS: usize = 5;
const MARKS: [(u32, u32); CHAPTERS] = [
    (0x49D, 0x4A2),
    (0x49E, 0x4A3),
    (0x49F, 0x4A4),
    (0x4A1, 0x4A6),
    (0x4A0, 0x4A5),
    (0x4CD, 0x4D1),
    (0x4CE, 0x4D2),
];
const MARK: f32 = 64.0;
const MARK_SHIFTS: u32 = 4;
/// The chapters' titles (`0x9E07CC`): 9000 to 9004, none for the last two.
const CHAPTER_TITLES: [Option<&str>; CHAPTERS] = [
    Some("9000"),
    Some("9001"),
    Some("9002"),
    Some("9003"),
    Some("9004"),
    None,
    None,
];
/// The left page's title, centred in (152, 160) 300 wide, and the right page's, centred and
/// wrapped in (540, 160) 316 wide, in red (`0xFF990000`).
const LEFT_TITLE: Rect = Rect {
    min: Vec2::new(152.0, 160.0),
    max: Vec2::new(452.0, 192.0),
};
const RIGHT_TITLE: Vec2 = Vec2::new(540.0, 160.0);
const RIGHT_TITLE_WIDTH: f32 = 316.0;
const TITLE_RED: u32 = 0xFF99_0000;
/// The list (`0x6B0240`): its rows from (160, 196), 284 wide, their texts 24 along and a side
/// quest's 16 further, each row taking its height and 1 more; a page ends before an entry once
/// its rows reach 334 down (`0x6B1D90`).
const LIST: Vec2 = Vec2::new(160.0, 196.0);
const LIST_WIDTH: f32 = 284.0;
const TEXT_IN: f32 = 24.0;
const SIDE_IN: f32 = 16.0;
const PAGE_DEPTH: f32 = 334.0;
const ROW_GAP: f32 = 1.0;
/// A row's click beside its icon, from 16 along, selects it.
const BESIDE_ICON: f32 = 16.0;
/// The ornament: two halves side by side (`GUI_main_08`), 25 high.
const ORNAMENT: [u32; 2] = [0xEE, 0xEF];
const ORNAMENT_HEIGHT: f32 = 25.0;
/// An entry's icon (`GUI_book_x3`), 16 × 16 at its row's (0, 2): a main quest's running, lost
/// and won, a side quest's tracked, running, lost and won.
const ICON_DOWN: f32 = 2.0;
const MAIN_ICONS: [u32; 3] = [0x496, 0x497, 0x498];
const SIDE_ICONS: [u32; 4] = [0x499, 0x49A, 0x49B, 0x49C];
/// A row's colours (`0x6B2B10`), each overriding the one before: its own, an ended quest's, a
/// new one's, the page's selected row's, and under the pointer, a new one's own.
const ROW: u32 = 0xFF00_0033;
const ENDED: u32 = 0xFF3F_3F3F;
const NEW: u32 = 0xFF66_0000;
const SELECTED: u32 = 0xFF00_0066;
const HOVERED: u32 = 0xFF00_0099;
const HOVERED_NEW: u32 = 0xFFFF_FF80;
/// The pages' counts, "page/pages", centred in (160, 576) 284 wide and (548, 576) 300 wide.
const COUNTS: [Rect; 2] = [
    Rect {
        min: Vec2::new(160.0, 576.0),
        max: Vec2::new(444.0, 608.0),
    },
    Rect {
        min: Vec2::new(548.0, 576.0),
        max: Vec2::new(848.0, 608.0),
    },
];
const BLUE: u32 = 0xFF00_0066;
/// The arrows, 32 × 32: the list's back and on, the text's back and on; the back's and the on's
/// images, dark green (`GUI_book_x3`'s lower row), and lit, yellow, under the pointer
/// (**seen**: the original's on arrow dark green with the pointer away).
const ARROWS: [Vec2; 4] = [
    Vec2::new(156.0, 581.0),
    Vec2::new(444.0, 581.0),
    Vec2::new(540.0, 581.0),
    Vec2::new(828.0, 581.0),
];
const ARROW: f32 = 32.0;
const BACK: (u32, u32) = (0x493, 0x492);
const ON: (u32, u32) = (0x495, 0x494);
/// The right page's text (`0x6B1FF0`): from x 548, 300 wide, below the title less 20, to 548
/// down; the picture of 256 × 256 at x 556 on its first page, its text 264 lower.
const BODY_X: f32 = 548.0;
const BODY_WIDTH: f32 = 300.0;
const BODY_FROM: f32 = 196.0;
const BODY_BOTTOM: f32 = 548.0;
const TITLE_OVER: f32 = 20.0;
const PICTURE_X: f32 = 556.0;
const PICTURE_ROOM: f32 = 264.0;
/// The picture's point, the iso plane's, from its place's (`0x6B0C33`, `0x89647C`, `0x896478`),
/// which the minimap shows at the screen's middle: the place stands near the picture's.
const PICTURE_SHIFT: Vec2 = Vec2::new(-2248.0, 420.0);
/// The lines' separator (`0x9E086C`) and the most lines an item shows.
const SEPARATOR: &str = "\n\n";
const ITEM_LINES: usize = 9;
/// The sounds of the book shown and hidden, and of a chapter or a tab selected (`(14, 2, 8)`,
/// `(14, 2, 9)`, `(14, 2, 10)`).
const CLOSES: u32 = 7104;
const TURNS: u32 = 7106;

/// The book's fonts (the table `0xA1CDB4`): 6, Carolingia 18, for the titles and the counts; 5,
/// Carolingia 16, for a main quest's row; 1, AntiquaSSK 13, for a side quest's row and the text.
/// Their sizes here, at which their words are about the original's width, their lines, the
/// height of an 'A' of the original's (**measured**: the text's lines 20 apart, the right page's
/// one line of title making its text start 8 lower), and how much lower than Bevy sets them the
/// original sets its letters in their lines (**measured** by the frames of #1560).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Style {
    Title,
    Main,
    Side,
    Body,
}

impl Style {
    fn font(self) -> &'static str {
        match self {
            Self::Title | Self::Main => MENU_FONT,
            Self::Side | Self::Body => FONT,
        }
    }

    fn size(self) -> f32 {
        match self {
            Self::Title => 26.0,
            Self::Main => 23.0,
            Self::Side | Self::Body => 19.5,
        }
    }

    fn line(self) -> f32 {
        match self {
            Self::Title => 28.0,
            Self::Main => 25.0,
            Self::Side | Self::Body => 20.0,
        }
    }

    fn drop(self) -> f32 {
        match self {
            Self::Main => 2.0,
            Self::Title | Self::Side | Self::Body => 3.0,
        }
    }

    /// The width it is wrapped in.
    fn width(self) -> f32 {
        match self {
            Self::Title => RIGHT_TITLE_WIDTH,
            Self::Main | Self::Side => LIST_WIDTH,
            Self::Body => BODY_WIDTH,
        }
    }
}

/// The quest book: whether it shows, its tab, the quests' book's chapter, its list's page and the
/// row selected on each page, the right page's page, the sets of the pages' parchment and the
/// bookmarks' shifts rolled as the game began, and the log's entries as the last build took them,
/// with their pictures' points.
#[derive(Resource, Debug, Default)]
pub struct Book {
    pub shown: bool,
    tab: usize,
    chapter: usize,
    page: usize,
    selected: HashMap<usize, usize>,
    text_page: usize,
    sheets: [u32; 2],
    shifts: [f32; CHAPTERS],
    entries: Vec<Entry>,
    places: Vec<Option<Vec2>>,
    /// Whether the book is to be built again from the log.
    stale: bool,
}

impl Book {
    /// A game's book, its rolls made by `dice` (`0x6AF440`, `0x6B3F90`).
    fn rolled(dice: &mut Rng) -> Self {
        let sheets = [dice.below(SHEET_SETS), dice.below(SHEET_SETS)];
        let shifts =
            std::array::from_fn(|_| f32::from(u16::try_from(dice.below(MARK_SHIFTS)).unwrap_or(0)));
        Self {
            sheets,
            shifts,
            ..Self::default()
        }
    }

    /// The row selected on the list's page `page`, 0 its ornament.
    fn selected(&self, page: usize) -> usize {
        self.selected.get(&page).copied().unwrap_or(0)
    }
}

/// A row of the list: an ornament, or the entry of the log's index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Row {
    Ornament,
    Entry(usize),
}

/// A chapter's place among the bookmarks: an entry's chapter, 0 taken as 1 (`0x6B07E0`).
fn chapter_of(entry: &Entry) -> Option<usize> {
    let chapter = usize::try_from(entry.chapter.max(1)).ok()?;
    Some(chapter - 1).filter(|&c| c < CHAPTERS)
}

/// The pages of the chapter `chapter`'s list (`0x6B07E0`, `0x6B1D90`): its entries with a title
/// in the log's order, an ornament first on each page and before a main quest whose entry follows
/// another kind's, a page ending before an entry once its rows reach [`PAGE_DEPTH`]; `height` is
/// a row's height by its entry.
fn pages(entries: &[Entry], chapter: usize, height: impl Fn(&Entry) -> f32) -> Vec<Vec<Row>> {
    let ornament = ORNAMENT_HEIGHT + ROW_GAP;
    let mut pages = vec![vec![Row::Ornament]];
    let mut used = ornament;
    for (index, entry) in entries.iter().enumerate() {
        if entry.title.is_none() || chapter_of(entry) != Some(chapter) {
            continue;
        }
        if used >= PAGE_DEPTH {
            pages.push(vec![Row::Ornament]);
            used = ornament;
        }
        let Some(page) = pages.last_mut() else {
            break;
        };
        let after_other = index > 0 && entries.get(index - 1).is_some_and(|before| !before.main());
        if entry.main() && after_other && page.last() != Some(&Row::Ornament) {
            page.push(Row::Ornament);
            used += ornament;
        }
        page.push(Row::Entry(index));
        used += height(entry) + ROW_GAP;
    }
    pages
}

/// The chapters that hold an entry, whose bookmarks show.
fn marked(entries: &[Entry]) -> [bool; CHAPTERS] {
    let mut marked = [false; CHAPTERS];
    for entry in entries.iter().filter(|entry| entry.title.is_some()) {
        if let Some(chapter) = chapter_of(entry) {
            marked[chapter] = true;
        }
    }
    marked
}

/// The style of an entry's row.
fn row_style(entry: &Entry) -> Style {
    if entry.main() {
        Style::Main
    } else {
        Style::Side
    }
}

/// An entry's icon: by its kind and how it stands, a side quest's by whether it is tracked.
fn icon(entry: &Entry, tracked: bool) -> u32 {
    match (entry.main(), entry.standing) {
        (true, Standing::Entered) => MAIN_ICONS[0],
        (true, Standing::Lost) => MAIN_ICONS[1],
        (true, Standing::Won) => MAIN_ICONS[2],
        (false, Standing::Entered) if tracked => SIDE_ICONS[0],
        (false, Standing::Entered) => SIDE_ICONS[1],
        (false, Standing::Lost) => SIDE_ICONS[2],
        (false, Standing::Won) => SIDE_ICONS[3],
    }
}

/// A row's colour (`0x6B2B10`).
fn row_colour(entry: &Entry, selected: bool, hovered: bool) -> u32 {
    let mut colour = ROW;
    if entry.standing != Standing::Entered {
        colour = ENDED;
    }
    if entry.new {
        colour = NEW;
    }
    if selected {
        colour = SELECTED;
    }
    if hovered {
        colour = if entry.new { HOVERED_NEW } else { HOVERED };
    }
    colour
}

/// The tab `tab`'s image, the hero's by the hero's type (`0x6AF250`).
fn tab_image(tab: usize, creature: u32) -> u32 {
    match tab {
        0 => QUESTS_TAB,
        1 => match creature {
            2 => 0x5B8,
            3 => 0x5BB,
            4 => 0x5BE,
            5 => 0x5B9,
            6 | 7 => 0x5BA,
            8 => 0x5C1,
            9 => 0x5C0,
            _ => 0x5B7,
        },
        2 => INFO_TAB,
        _ => STATS_TAB,
    }
}

/// The corner of the tab `tab`.
fn tab_at(tab: usize, selected: usize) -> Vec2 {
    let i = f32::from(u8::try_from(tab).unwrap_or(0));
    let y = if tab == selected { TAB_RAISED } else { TAB_Y };
    Vec2::new(TAB_X + TAB_STEP * i, y)
}

/// The corner of the chapter `chapter`'s bookmark, moved left by `shift`.
fn mark_at(chapter: usize, shift: f32) -> Vec2 {
    let (x, row) = if chapter < LEFT_MARKS {
        (86.0, chapter)
    } else {
        (866.0, chapter - LEFT_MARKS)
    };
    Vec2::new(
        x - shift,
        184.0 + 80.0 * f32::from(u8::try_from(row).unwrap_or(0)),
    )
}

/// The lines measured of the texts laid out, by their styles.
#[derive(Resource, Default)]
struct Measures(HashMap<(Style, String), usize>);

impl Measures {
    /// The lines of `text` in `style`, one while it is not measured.
    fn lines(&self, style: Style, text: &str) -> usize {
        self.0
            .get(&(style, text.to_owned()))
            .copied()
            .unwrap_or(1)
            .max(1)
    }

    fn has(&self, style: Style, text: &str) -> bool {
        text.is_empty() || self.0.contains_key(&(style, text.to_owned()))
    }
}

/// A hidden text laid out to be measured.
#[derive(Component)]
struct Measuring(Style, String);

/// The container of the hidden texts.
#[derive(Component)]
struct MeasureRoot;

/// The book's container.
#[derive(Component)]
struct BookRoot;

/// What the pointer is over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Over {
    Tab(usize),
    Close,
    Mark(usize),
    Arrow(usize),
    /// A row of the list's page, and whether beside its icon.
    Row(usize, bool),
}

/// The book's model: its texts and their pages, as the entries and the measures make them.
struct Model {
    /// The chapters' pages, the selected chapter's rows.
    pages: Vec<Vec<Row>>,
    marked: [bool; CHAPTERS],
    /// Each row's top, of the list's page shown, and its text's lines.
    rows: Vec<(f32, Row, usize)>,
    /// The entry whose text the right page shows.
    item: Option<usize>,
    /// The right page's title, its lines, its text, and its pages.
    title: String,
    title_lines: usize,
    text: String,
    text_pages: usize,
    /// The right page's first page's lines and every later page's.
    first: usize,
    later: usize,
}

/// The text of the id `id`, its line breaks (`<n>`) broken and its colours' tags left out.
fn said(texts: Option<&Texts>, id: u32) -> String {
    texts
        .and_then(|texts| texts.id(id))
        .map_or_else(|| id.to_string(), crate::talk::shown_text)
}

/// The text an entry's item pours on the right page (`0x6B07E0`): its first line, then its others
/// without repeats among them, nine at most, a blank line between each two.
fn item_text(entry: &Entry, texts: Option<&Texts>) -> String {
    let mut lines: Vec<u32> = entry.lines.first().copied().into_iter().collect();
    let mut further: Vec<u32> = Vec::new();
    for &line in entry.lines.iter().skip(1) {
        if !further.contains(&line) {
            further.push(line);
        }
    }
    lines.extend(further);
    lines.truncate(ITEM_LINES);
    lines
        .into_iter()
        .map(|line| said(texts, line))
        .collect::<Vec<_>>()
        .join(SEPARATOR)
}

impl Model {
    fn of(book: &Book, texts: Option<&Texts>, measures: &Measures) -> Self {
        let entries = &book.entries;
        let row_lines = |entry: &Entry| {
            let title = entry.title.map(|id| said(texts, id)).unwrap_or_default();
            measures.lines(row_style(entry), &title)
        };
        let height = |entry: &Entry| {
            f32::from(u16::try_from(row_lines(entry)).unwrap_or(1)) * row_style(entry).line()
        };
        let pages = if book.tab == 0 {
            pages(entries, book.chapter, height)
        } else {
            vec![Vec::new()]
        };
        let page = book.page.min(pages.len().saturating_sub(1));
        let mut rows = Vec::new();
        let mut y = LIST.y;
        for &row in pages.get(page).into_iter().flatten() {
            let (lines, h) = match row {
                Row::Ornament => (0, ORNAMENT_HEIGHT),
                Row::Entry(index) => entries
                    .get(index)
                    .map_or((1, 0.0), |entry| (row_lines(entry), height(entry))),
            };
            rows.push((y, row, lines));
            y += h + ROW_GAP;
        }
        // The selected row's entry, the ornament's the chapter's first.
        let first_entry = pages.first().and_then(|rows| {
            rows.iter().find_map(|row| match row {
                Row::Entry(index) => Some(*index),
                Row::Ornament => None,
            })
        });
        let item = match rows.get(book.selected(page)) {
            Some((_, Row::Entry(index), _)) => Some(*index),
            _ => first_entry,
        };
        let entry = item.and_then(|index| entries.get(index));
        let title = entry
            .and_then(|entry| entry.title)
            .map(|id| said(texts, id))
            .unwrap_or_default();
        let text = entry
            .map(|entry| item_text(entry, texts))
            .unwrap_or_default();
        let title_lines = if title.is_empty() {
            0
        } else {
            measures.lines(Style::Title, &title)
        };
        let text_lines = if text.is_empty() {
            0
        } else {
            measures.lines(Style::Body, &text)
        };
        let (_, height) = body(title_lines);
        let picture = item
            .and_then(|index| book.places.get(index).copied().flatten())
            .is_some();
        let fits = |height: f32| {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a page's lines, a few, of a height held above 0"
            )]
            let lines = (height.max(0.0) / Style::Body.line()).floor() as usize;
            lines
        };
        let later = fits(height).max(1);
        let first = if picture {
            fits(height - PICTURE_ROOM)
        } else {
            later
        };
        let text_pages = if text_lines <= first {
            1
        } else {
            1 + (text_lines - first).div_ceil(later)
        };
        Self {
            marked: marked(entries),
            pages,
            rows,
            item,
            title,
            title_lines,
            text,
            text_pages,
            first,
            later,
        }
    }

    /// The texts it lays out that are not measured yet.
    fn unmeasured(
        &self,
        book: &Book,
        texts: Option<&Texts>,
        measures: &Measures,
    ) -> Vec<(Style, String)> {
        let mut wanted = Vec::new();
        for page in &self.pages {
            for row in page {
                if let Row::Entry(index) = row
                    && let Some(entry) = book.entries.get(*index)
                {
                    let title = entry.title.map(|id| said(texts, id)).unwrap_or_default();
                    wanted.push((row_style(entry), title));
                }
            }
        }
        wanted.push((Style::Title, self.title.clone()));
        wanted.push((Style::Body, self.text.clone()));
        wanted.retain(|(style, text)| !measures.has(*style, text));
        wanted.dedup();
        wanted
    }
}

/// The right page's text's top and height under a title of `title_lines` (`0x6B1FF0`).
fn body(title_lines: usize) -> (f32, f32) {
    let title = f32::from(u16::try_from(title_lines.max(1)).unwrap_or(1)) * Style::Title.line();
    let top = BODY_FROM + title - TITLE_OVER;
    (top, BODY_BOTTOM - top)
}

/// What the pointer is over on a book that shows.
fn over(book: &Book, model: &Model, at: Vec2) -> Option<Over> {
    let within = |corner: Vec2, size: Vec2| Rect::from_corners(corner, corner + size).contains(at);
    if within(CLOSE.1, Vec2::splat(CLOSE_SIDE)) {
        return Some(Over::Close);
    }
    if let Some(tab) = (0..TABS).find(|&tab| within(tab_at(tab, book.tab), TAB_SIZE)) {
        return Some(Over::Tab(tab));
    }
    if book.tab == 0
        && let Some(chapter) = (0..CHAPTERS).find(|&chapter| {
            model.marked[chapter]
                && within(mark_at(chapter, book.shifts[chapter]), Vec2::splat(MARK))
        })
    {
        return Some(Over::Mark(chapter));
    }
    if let Some(arrow) = (0..ARROWS.len())
        .find(|&arrow| arrow_shown(book, model, arrow) && within(ARROWS[arrow], Vec2::splat(ARROW)))
    {
        return Some(Over::Arrow(arrow));
    }
    // A row within its first line, across the list (`0x6B3640`).
    if (LIST.x..LIST.x + LIST_WIDTH).contains(&at.x) {
        for (i, (top, row, _)) in model.rows.iter().enumerate() {
            let Row::Entry(index) = row else {
                continue;
            };
            let line = book
                .entries
                .get(*index)
                .map_or(0.0, |entry| row_style(entry).line());
            if (*top..*top + line - 1.0).contains(&at.y) {
                return Some(Over::Row(i, at.x >= LIST.x + BESIDE_ICON));
            }
        }
    }
    None
}

/// Whether the arrow `arrow` shows: the list's back above its first page and on below its last,
/// the text's likewise.
fn arrow_shown(book: &Book, model: &Model, arrow: usize) -> bool {
    let (page, pages) = if arrow < 2 {
        (book.page, model.pages.len())
    } else {
        (book.text_page, model.text_pages)
    };
    if arrow.is_multiple_of(2) {
        page > 0
    } else {
        page + 1 < pages
    }
}

/// A book sound.
fn play(sounds: &mut MessageWriter<sfx::Play>, sound: u32) {
    sounds.write(sfx::Play {
        sound: sfx::Of::Id(sound),
        at: None,
        rate: (0.5, sfx::Roll::None),
    });
}

/// The book built again from the log (`0x6B1990`): the entries and their places taken as they
/// stand, the chapter selected again with its sound, the list's page and its selection kept where
/// they still stand.
fn rebuild(
    book: &mut Book,
    live: Option<&crate::scripts::Live>,
    place: impl Fn(&opensacred_rules::quests::Compass) -> Option<Vec2>,
    sounds: &mut MessageWriter<sfx::Play>,
) {
    let Some(live) = live else {
        return;
    };
    let quests = live.quests();
    book.entries = quests.book().to_vec();
    book.places = book
        .entries
        .iter()
        .map(|entry| quests.compass(entry.quest).and_then(&place))
        .collect();
    debug!(
        "quest book: built of {} entries, their places {:?}",
        book.entries.len(),
        book.places
    );
    let kept = book.selected(book.page);
    book.selected.clear();
    book.selected.insert(book.page, kept);
    play(sounds, TURNS);
}

/// `L`, the bar's book button and Esc show and hide the book, with their sounds; the book's show
/// hides the inventory, and the inventory's show the book; neither while the world map shows
/// (`0x757840`).
#[expect(
    clippy::too_many_arguments,
    reason = "the keys, the click, the windows it hides and its sounds"
)]
pub(crate) fn toggle(
    mut keys: crate::controls::KeysMut,
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    images: Res<UiImages>,
    mut book: ResMut<Book>,
    mut inventory: ResMut<crate::inventory::Shown>,
    mut sounds: MessageWriter<sfx::Play>,
    map: Res<crate::world_map::Map>,
) {
    if map.shown {
        return;
    }
    let pressed = keys.take(crate::controls::Action::QuestBook);
    let clicked = buttons.just_pressed(MouseButton::Left)
        && screen_point(&pointer, &ui_scale, &windows).is_some_and(|at| {
            let (index, corner) = bar::BOOK_BUTTON;
            Rect::from_corners(corner, corner + images.size(index)).contains(at)
        });
    let escaped = book.shown && keys.take(crate::controls::Action::Back);
    if !(pressed || clicked || escaped) {
        return;
    }
    book.shown = !book.shown;
    play(&mut sounds, CLOSES);
    if book.shown {
        book.stale = true;
        if inventory.0 {
            inventory.0 = false;
            play(&mut sounds, crate::inventory::CLOSES);
        }
    }
}

/// The inventory's show hides the book, with its sound.
fn hide_under_inventory(
    inventory: Res<crate::inventory::Shown>,
    mut book: ResMut<Book>,
    mut sounds: MessageWriter<sfx::Play>,
) {
    if inventory.is_changed() && inventory.0 && book.shown {
        book.shown = false;
        play(&mut sounds, CLOSES);
    }
}

/// The book built again as it shows.
fn build(
    mut book: ResMut<Book>,
    live: Option<NonSend<crate::scripts::Live>>,
    named: Query<(&crate::npcs::ScriptName, &GlobalTransform)>,
    mut sounds: MessageWriter<sfx::Play>,
) {
    if !book.stale {
        return;
    }
    book.stale = false;
    // The compass's cell (`0x497540`), its middle on the iso plane, moved by (−2248, 420) and
    // truncated (`0x6B0C33`).
    let place = |compass: &opensacred_rules::quests::Compass| {
        let at = crate::compass::target_of(compass, &named)?;
        let middle = crate::paths::centre(crate::paths::cell_of(at.x, at.y));
        Some((crate::minimap::iso(middle) + PICTURE_SHIFT).trunc())
    };
    rebuild(&mut book, live.as_deref(), place, &mut sounds);
}

/// The clicks on the book: its tabs, its close button, its bookmarks, its arrows and its rows.
fn click(
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    mut book: ResMut<Book>,
    (texts, measures): (Option<Res<Texts>>, Res<Measures>),
    mut live: Option<NonSendMut<crate::scripts::Live>>,
    mut sounds: MessageWriter<sfx::Play>,
) {
    let (left, right) = (
        buttons.just_pressed(MouseButton::Left),
        buttons.just_pressed(MouseButton::Right),
    );
    if !book.shown || !(left || right) {
        return;
    }
    let Some(at) = screen_point(&pointer, &ui_scale, &windows) else {
        return;
    };
    let model = Model::of(&book, texts.as_deref(), &measures);
    let Some(over) = over(&book, &model, at) else {
        return;
    };
    match (over, left) {
        (Over::Tab(tab), true) => {
            book.tab = tab;
            play(&mut sounds, TURNS);
            if tab < TABS - 1 {
                book.stale = true;
            }
        }
        (Over::Close, true) => {
            book.shown = false;
            play(&mut sounds, CLOSES);
        }
        (Over::Mark(chapter), true) => {
            if chapter != book.chapter {
                book.page = 0;
            }
            book.chapter = chapter;
            book.text_page = 0;
            play(&mut sounds, TURNS);
        }
        (Over::Arrow(arrow), true) => {
            let (page, pages) = if arrow < 2 {
                (&mut book.page, model.pages.len())
            } else {
                (&mut book.text_page, model.text_pages)
            };
            if arrow.is_multiple_of(2) {
                *page = page.saturating_sub(1);
            } else if *page + 1 < pages {
                *page += 1;
            }
            if arrow < 2 {
                book.text_page = 0;
            }
        }
        (Over::Row(i, beside), _) => {
            let Some((_, Row::Entry(index), _)) = model.rows.get(i).copied() else {
                return;
            };
            let Some((quest, main, standing)) = book
                .entries
                .get(index)
                .map(|entry| (entry.quest, entry.main(), entry.standing))
            else {
                return;
            };
            // A side quest that runs tracked, the book built again (`0x49DAB0`).
            let tracks = book.tab == 0 && !main && standing == Standing::Entered;
            if tracks && let Some(live) = live.as_deref_mut() {
                live.track(quest);
                info!("quest book: the side quest {quest} tracked");
                book.stale = true;
            }
            if (left && beside) || tracks {
                let page = book.page;
                book.selected.insert(page, i);
                book.text_page = 0;
            }
        }
        _ => {}
    }
}

/// Lays out the texts not measured yet in hidden nodes, and takes their lines once laid out.
fn measure(
    mut commands: Commands,
    book: Res<Book>,
    (texts, mut measures): (Option<Res<Texts>>, ResMut<Measures>),
    assets: Res<AssetServer>,
    measuring: Query<(Entity, &Measuring, &ComputedNode)>,
    roots: Query<Entity, With<MeasureRoot>>,
) {
    for (entity, Measuring(style, text), node) in &measuring {
        let height = node.size().y * node.inverse_scale_factor;
        if height <= 0.0 {
            continue;
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a text's lines, a few, of a height above 0"
        )]
        let lines = (height / style.line()).round() as usize;
        measures.0.insert((*style, text.clone()), lines.max(1));
        commands.entity(entity).despawn();
    }
    if !book.shown {
        return;
    }
    let model = Model::of(&book, texts.as_deref(), &measures);
    let wanted = model.unmeasured(&book, texts.as_deref(), &measures);
    let pending: Vec<(Style, String)> = measuring
        .iter()
        .map(|(_, Measuring(style, text), _)| (*style, text.clone()))
        .collect();
    let root = match roots.iter().next() {
        Some(root) => root,
        None => commands
            .spawn((
                MeasureRoot,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    ..default()
                },
                Visibility::Hidden,
                Pickable::IGNORE,
                DespawnOnExit(AppState::InGame),
            ))
            .id(),
    };
    for (style, text) in wanted {
        if pending.contains(&(style, text.clone())) {
            continue;
        }
        let child = commands
            .spawn((
                Measuring(style, text.clone()),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(style.width()),
                    ..default()
                },
                Text::new(text),
                lettering(&assets, style),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(root).add_child(child);
    }
}

/// A style's font, size and line.
fn lettering(assets: &AssetServer, style: Style) -> (TextFont, LineHeight) {
    (
        TextFont {
            font: assets.load(style.font()).into(),
            font_size: FontSize::Px(style.size()),
            ..default()
        },
        LineHeight::Px(style.line()),
    )
}

/// What the book shows, made again as it changes.
#[derive(Clone, Debug, PartialEq)]
struct Look {
    tab: usize,
    chapter: usize,
    page: usize,
    selected: usize,
    text_page: usize,
    over: Option<Over>,
    creature: u32,
    entries: Vec<Entry>,
    tracked: Option<u32>,
    measured: usize,
}

/// Makes the book as it shows and as it changes, and takes it away as it hides.
#[expect(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    reason = "the book's pieces read best in their order"
)]
fn show(
    mut commands: Commands,
    book: Res<Book>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    (images, assets, texts): (Res<UiImages>, Res<AssetServer>, Option<Res<Texts>>),
    (measures, minimaps): (Res<Measures>, Res<crate::minimap::Minimaps>),
    hero: Option<Res<crate::hero::Hero>>,
    live: Option<NonSend<crate::scripts::Live>>,
    roots: Query<Entity, With<BookRoot>>,
    mut was: Local<Option<Look>>,
) {
    let texts = texts.as_deref();
    let model = book.shown.then(|| Model::of(&book, texts, &measures));
    let now = model.as_ref().map(|model| Look {
        tab: book.tab,
        chapter: book.chapter,
        page: book.page,
        selected: book.selected(book.page),
        text_page: book.text_page,
        over: screen_point(&pointer, &ui_scale, &windows).and_then(|at| over(&book, model, at)),
        creature: hero.as_deref().map_or(1, |hero| hero.creature),
        entries: book.entries.clone(),
        tracked: live.as_deref().and_then(|live| live.quests().tracked()),
        measured: measures.0.len(),
    });
    if *was == now {
        return;
    }
    *was = now.clone();
    for root in &roots {
        commands.entity(root).despawn();
    }
    let (Some(look), Some(model)) = (now, model) else {
        return;
    };
    let root = commands
        .spawn((BookRoot, crate::backpack::screen(), GlobalZIndex(2)))
        .id();
    let add = |commands: &mut Commands, bundle: (ImageNode, Node)| {
        let child = commands.spawn((bundle, Pickable::IGNORE)).id();
        commands.entity(root).add_child(child);
    };
    let piece = |index: u32, at: Vec2| bar::piece(&images, &assets, index, at, Vec2::ZERO);
    // The picture, the tabs and the bookmarks under the parchment.
    for (i, name) in BASES.iter().enumerate() {
        let Some(file) = images.file(name) else {
            continue;
        };
        let i = u8::try_from(i).unwrap_or(0);
        let at = BASE_AT + BASE * Vec2::new(f32::from(i % 3), f32::from(i / 3));
        add(
            &mut commands,
            (
                ImageNode::new(crate::hud::image(&assets, file.to_owned())),
                bar::placed(at, Vec2::splat(BASE)),
            ),
        );
    }
    for tab in 0..TABS {
        add(
            &mut commands,
            piece(tab_image(tab, look.creature), tab_at(tab, look.tab)),
        );
    }
    if look.tab == 0 {
        for chapter in (0..CHAPTERS).filter(|&chapter| model.marked[chapter]) {
            let (image, lit) = MARKS[chapter];
            let shown = if chapter == look.chapter || look.over == Some(Over::Mark(chapter)) {
                lit
            } else {
                image
            };
            add(
                &mut commands,
                piece(shown, mark_at(chapter, book.shifts[chapter])),
            );
        }
    }
    for (page, x) in PAGES_X.into_iter().enumerate() {
        let first = SHEETS_FROM + 4 * book.sheets[page];
        for (k, at) in (0..4).zip(SHEET_AT) {
            add(
                &mut commands,
                piece(first + k, Vec2::new(x, BASE_AT.y) + at),
            );
        }
    }
    add(&mut commands, piece(CLOSE.0, CLOSE.1));
    if look.tab >= TABS - 1 {
        return;
    }
    let write = |commands: &mut Commands,
                 (text, rect): (&str, Rect),
                 (style, colour, centred): (Style, u32, bool)| {
        let mut node = bar::placed(rect.min + Vec2::new(0.0, style.drop()), rect.size());
        node.height = Val::Auto;
        let child = commands
            .spawn((
                node,
                Text::new(text),
                lettering(&assets, style),
                TextColor(argb(colour)),
                TextLayout {
                    justify: if centred {
                        Justify::Center
                    } else {
                        Justify::Left
                    },
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(root).add_child(child);
        child
    };
    // The left page: the chapter's title, the list's rows, its arrows and its count.
    if look.tab == 0
        && let Some(key) = CHAPTER_TITLES[look.chapter]
    {
        let title = texts.and_then(|texts| texts.named(key)).unwrap_or(key);
        write(
            &mut commands,
            (title, LEFT_TITLE),
            (Style::Title, TITLE_RED, true),
        );
    }
    let tracked = look.tracked;
    for (i, &(top, row, _)) in model.rows.iter().enumerate() {
        match row {
            Row::Ornament => {
                let left = images.size(ORNAMENT[0]).x;
                add(&mut commands, piece(ORNAMENT[0], Vec2::new(LIST.x, top)));
                add(
                    &mut commands,
                    piece(ORNAMENT[1], Vec2::new(LIST.x + left, top)),
                );
            }
            Row::Entry(index) => {
                let Some(entry) = book.entries.get(index) else {
                    continue;
                };
                add(
                    &mut commands,
                    piece(
                        icon(entry, Some(entry.quest) == tracked),
                        Vec2::new(LIST.x, top + ICON_DOWN),
                    ),
                );
                let style = row_style(entry);
                let x = LIST.x + TEXT_IN + if entry.main() { 0.0 } else { SIDE_IN };
                let colour = row_colour(
                    entry,
                    look.selected == i,
                    matches!(look.over, Some(Over::Row(r, _)) if r == i),
                );
                let title = entry.title.map(|id| said(texts, id)).unwrap_or_default();
                write(
                    &mut commands,
                    (&title, Rect::new(x, top, x + LIST_WIDTH, top)),
                    (style, colour, false),
                );
            }
        }
    }
    let count = |page: usize, pages: usize| format!("{}/{}", page + 1, pages.max(1));
    write(
        &mut commands,
        (&count(look.page, model.pages.len()), COUNTS[0]),
        (Style::Title, BLUE, true),
    );
    // The right page: its title, its picture, its text's page and its count.
    if !model.title.is_empty() {
        let rect = Rect::new(
            RIGHT_TITLE.x,
            RIGHT_TITLE.y,
            RIGHT_TITLE.x + RIGHT_TITLE_WIDTH,
            RIGHT_TITLE.y,
        );
        write(
            &mut commands,
            (&model.title, rect),
            (Style::Title, TITLE_RED, true),
        );
    }
    let (top, _) = body(model.title_lines);
    let place = model
        .item
        .and_then(|index| book.places.get(index).copied().flatten());
    let line = Style::Body.line();
    let lines = |n: usize| f32::from(u16::try_from(n).unwrap_or(0)) * line;
    let text_page = look.text_page.min(model.text_pages.saturating_sub(1));
    let (from, shown, skipped) = match (text_page, place) {
        (0, Some(place)) => {
            let origin = Vec2::new(PICTURE_X, top);
            let mut node = bar::placed(origin, Vec2::splat(crate::minimap::PICTURE));
            node.overflow = Overflow::clip();
            let picture = commands.spawn((node, Pickable::IGNORE)).id();
            commands.entity(root).add_child(picture);
            crate::minimap::spawn(
                &mut commands,
                picture,
                (&minimaps, &assets),
                (place, origin),
                1.0,
            );
            (top + PICTURE_ROOM, model.first, 0)
        }
        (0, None) => (top, model.later, 0),
        (page, _) => (top, model.later, model.first + (page - 1) * model.later),
    };
    if !model.text.is_empty() {
        let mut clip = bar::placed(
            Vec2::new(BODY_X, from + Style::Body.drop()),
            Vec2::new(BODY_WIDTH, lines(shown)),
        );
        clip.overflow = Overflow::clip();
        let clip = commands.spawn((clip, Pickable::IGNORE)).id();
        commands.entity(root).add_child(clip);
        let mut node = bar::placed(Vec2::new(0.0, -lines(skipped)), Vec2::new(BODY_WIDTH, 0.0));
        node.height = Val::Auto;
        let text = commands
            .spawn((
                node,
                Text::new(model.text.clone()),
                lettering(&assets, Style::Body),
                TextColor(argb(BLUE)),
                // Justified, a paragraph's last line set left (the flags `0x2500`, **seen**).
                TextLayout {
                    justify: Justify::Justified,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(clip).add_child(text);
    }
    write(
        &mut commands,
        (&count(text_page, model.text_pages), COUNTS[1]),
        (Style::Title, BLUE, true),
    );
    for (arrow, at) in ARROWS.into_iter().enumerate() {
        if !arrow_shown(&book, &model, arrow) {
            continue;
        }
        let (image, lit) = if arrow.is_multiple_of(2) { BACK } else { ON };
        let shown = if look.over == Some(Over::Arrow(arrow)) {
            lit
        } else {
            image
        };
        add(&mut commands, piece(shown, at));
    }
}

/// The tabs' tooltips (the style `0x20B`), at the tab's top, 32 along.
fn tooltip(
    mut commands: Commands,
    time: Res<Time<Real>>,
    book: Res<Book>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    (assets, texts): (Res<AssetServer>, Option<Res<Texts>>),
    tips: Query<Entity, With<TipRoot>>,
    mut rest: Local<Rest>,
) {
    let at = book
        .shown
        .then(|| screen_point(&pointer, &ui_scale, &windows))
        .flatten();
    let over = at.and_then(|at| {
        (0..TABS).find(|&tab| {
            let corner = tab_at(tab, book.tab);
            Rect::from_corners(corner, corner + TAB_SIZE).contains(at)
        })
    });
    let (gone, due) = rest.step(
        over.and_then(|tab| u32::try_from(tab).ok()),
        time.elapsed_secs(),
    );
    if gone {
        for tip in &tips {
            commands.entity(tip).despawn();
        }
    }
    let Some(tab) = due.and_then(|tab| usize::try_from(tab).ok()) else {
        return;
    };
    if let Some(text) = texts
        .as_deref()
        .and_then(|texts| texts.named(TAB_TIPS[tab]))
    {
        let corner = tab_at(tab, book.tab);
        spawn_tip(
            &mut commands,
            &assets,
            text,
            corner + Vec2::new(TAB_SIZE.x / 2.0, 0.0),
        );
    }
}

/// Whether the pointer is off the book: the world's clicks pass where it is.
fn off_the_book(
    book: Res<Book>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
) -> bool {
    if !book.shown {
        return true;
    }
    let Some(at) = screen_point(&pointer, &ui_scale, &windows) else {
        return true;
    };
    let marks = marked(&book.entries);
    let on_mark = book.tab == 0
        && (0..CHAPTERS).any(|chapter| {
            let corner = mark_at(chapter, book.shifts[chapter]);
            marks[chapter] && Rect::from_corners(corner, corner + Vec2::splat(MARK)).contains(at)
        });
    !(TAKES.contains(at) || on_mark)
}

/// A new game's book, hidden, its rolls made.
fn reset(mut commands: Commands) {
    commands.insert_resource(Book::rolled(&mut Rng::of(0, 0, 1560)));
}

/// The quest book.
pub struct QuestBookPlugin;

impl Plugin for QuestBookPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Book>()
            .init_resource::<Measures>()
            .configure_sets(Update, Control.run_if(off_the_book))
            .add_systems(OnEnter(AppState::InGame), reset)
            .add_systems(
                PreUpdate,
                toggle
                    .after(InputSystems)
                    .after(crate::console::keys)
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                Update,
                (hide_under_inventory, build, click, measure, show, tooltip)
                    .chain()
                    .after(crate::inventory::Opened)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(quest: u32, chapter: i32) -> Entry {
        Entry {
            quest,
            title: Some(quest),
            chapter,
            ..Entry::default()
        }
    }

    #[test]
    fn a_chapters_list_puts_an_ornament_first_and_before_a_main_quest_after_a_side_one() {
        let entries = [
            entry(1, 1),
            entry(150, 1),
            entry(2, 1),
            entry(3, 2),
            Entry {
                title: None,
                ..entry(4, 1)
            },
            entry(151, 0),
        ];
        let pages = pages(&entries, 0, |_| 25.0);
        assert_eq!(
            pages,
            [vec![
                Row::Ornament,
                Row::Entry(0),
                Row::Entry(1),
                Row::Ornament,
                Row::Entry(2),
                Row::Entry(5),
            ]]
        );
        assert_eq!(
            marked(&entries),
            [true, true, false, false, false, false, false]
        );
    }

    #[test]
    fn a_page_of_the_list_ends_once_its_rows_reach_334_down() {
        let entries: Vec<Entry> = (150..170).map(|quest| entry(quest, 1)).collect();
        // The ornament's 26, then rows of 21: twelve reach 278, fourteen 320, fifteen 341.
        let pages = pages(&entries, 0, |_| 20.0);
        assert_eq!(pages[0].len(), 1 + 15);
        assert_eq!(pages[1][0], Row::Ornament);
        assert_eq!(pages[1][1], Row::Entry(15));
    }

    #[test]
    fn a_rows_colour_follows_how_it_stands_and_the_pointer() {
        let mut quest = entry(150, 1);
        assert_eq!(row_colour(&quest, false, false), ROW);
        quest.standing = Standing::Won;
        assert_eq!(row_colour(&quest, false, false), ENDED);
        assert_eq!(icon(&quest, false), 0x49C);
        quest.new = true;
        assert_eq!(row_colour(&quest, false, false), NEW);
        assert_eq!(row_colour(&quest, true, false), SELECTED);
        assert_eq!(row_colour(&quest, true, true), HOVERED_NEW);
        quest.standing = Standing::Entered;
        assert_eq!((icon(&quest, true), icon(&quest, false)), (0x499, 0x49A));
        assert_eq!(icon(&entry(1, 1), false), 0x496);
    }

    #[test]
    fn an_items_text_takes_its_first_line_and_its_others_once_each() {
        let quest = Entry {
            lines: vec![7, 8, 7, 9, 8],
            ..entry(1, 1)
        };
        assert_eq!(item_text(&quest, None), "7\n\n8\n\n7\n\n9");
    }

    #[test]
    fn the_hero_tab_shows_the_heros_class() {
        assert_eq!(tab_image(1, 1), 0x5B7);
        assert_eq!(tab_image(1, 7), 0x5BA);
        assert_eq!(tab_image(1, 9), 0x5C0);
        assert_eq!(tab_at(2, 2), Vec2::new(308.0, 72.0));
        assert_eq!(mark_at(5, 3.0), Vec2::new(863.0, 184.0));
        assert_eq!(mark_at(4, 0.0), Vec2::new(86.0, 504.0));
    }
}
