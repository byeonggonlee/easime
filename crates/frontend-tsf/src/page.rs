//! What a candidate window shows, the same as the Linux window
//! (`frontend-wayland/src/candidate.rs`): the reading at the top, the page of
//! candidates numbered 1 to 9 with the chosen one marked (by a sign as well as
//! a colour), and at the bottom the page and the keys. The Windows window draws
//! this ([`crate::candidate`]); applications that draw candidates themselves
//! get the same list.

use ime_core::CandidateList;

/// One line of the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// What to press to choose it, 1 to 9 on every page.
    pub number: usize,
    pub text: String,
    /// The meaning of a Korean Hanja, if any.
    pub annotation: Option<String>,
    pub selected: bool,
}

impl Row {
    /// The number, with a sign when this is the chosen candidate.
    pub fn label(&self) -> String {
        if self.selected {
            format!("> {}", self.number)
        } else {
            format!("  {}", self.number)
        }
    }
}

/// The page of candidates that holds the chosen one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// What is being converted.
    pub reading: String,
    pub rows: Vec<Row>,
    /// The page and the keys.
    pub footer: String,
}

/// The page to show for `list`, or `None` if there is nothing to choose.
/// `composition` is what the document shows: the reading when the list does
/// not give one (Korean Hanja converts the syllable being composed).
pub fn page(list: &CandidateList, composition: &str) -> Option<Page> {
    if list.items.is_empty() || list.selected >= list.items.len() {
        return None;
    }
    let reading = if list.reading.is_empty() {
        composition.to_owned()
    } else {
        list.reading.clone()
    };
    let rows = (list.page_start()..list.page_end())
        .enumerate()
        .map(|(row, index)| Row {
            number: row + 1,
            text: list.items[index].clone(),
            annotation: list
                .annotations
                .get(index)
                .filter(|annotation| !annotation.is_empty())
                .cloned(),
            selected: index == list.selected,
        })
        .collect();
    let footer = format!(
        "{}/{}  PgUp/PgDn  1-9/Enter  Esc",
        list.page_number(),
        list.page_count()
    );
    Some(Page {
        reading,
        rows,
        footer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(count: usize, selected: usize) -> CandidateList {
        CandidateList {
            items: (1..=count).map(|n| format!("候補{n}")).collect(),
            selected,
            annotations: Vec::new(),
            reading: "こうほ".into(),
        }
    }

    #[test]
    fn the_page_of_the_chosen_candidate_is_numbered_from_one() {
        let page = page(&list(12, 10), "候補11").unwrap();
        assert_eq!(page.reading, "こうほ");
        let texts: Vec<&str> = page.rows.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts, ["候補10", "候補11", "候補12"]);
        assert_eq!(page.rows[0].number, 1);
        assert_eq!(page.rows[1].label(), "> 2", "the chosen one is marked");
        assert_eq!(page.rows[0].label(), "  1");
        assert_eq!(page.footer, "2/2  PgUp/PgDn  1-9/Enter  Esc");
    }

    #[test]
    fn korean_hanja_show_the_syllable_and_the_meanings() {
        let hanja = CandidateList {
            items: vec!["韓".into(), "漢".into()],
            selected: 0,
            annotations: vec!["나라 이름 한".into(), String::new()],
            reading: String::new(),
        };
        let page = page(&hanja, "한").unwrap();
        assert_eq!(page.reading, "한");
        assert_eq!(page.rows[0].annotation.as_deref(), Some("나라 이름 한"));
        assert_eq!(
            page.rows[1].annotation, None,
            "an empty meaning is no meaning"
        );
        assert_eq!(page.footer, "1/1  PgUp/PgDn  1-9/Enter  Esc");
    }

    #[test]
    fn nothing_to_choose_shows_no_page() {
        assert_eq!(page(&list(0, 0), ""), None);
        assert_eq!(page(&list(2, 5), ""), None);
    }
}
