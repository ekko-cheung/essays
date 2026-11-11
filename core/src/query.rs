use crate::note::NoteSummary;
use crate::{Db, Result};
use rusqlite::types::Value;
use rusqlite::params_from_iter;
use serde::Deserialize;
use std::collections::HashMap;

/// Combined note filter. Empty / `None` fields mean "don't filter on this".
///
/// Semantics (chosen to match how people narrow things down):
/// - status: OR — a note matches if it has *any* of the selected statuses
///   (`no_status` adds "notes without a status" to that set)
/// - tags: AND — a note must carry *all* selected tags
/// - search: every whitespace-separated word must appear in title or body
/// - all groups are ANDed together
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NoteQuery {
    /// `None` = all notebooks.
    pub notebook_id: Option<String>,
    /// Also include notes of nested sub-notebooks (ignored when `notebook_id` is None).
    pub include_descendants: bool,
    pub status_ids: Vec<String>,
    pub no_status: bool,
    pub tag_ids: Vec<String>,
    pub search: String,
}

/// Escape LIKE wildcards so user input is matched literally.
fn like_pattern(word: &str) -> String {
    let mut out = String::from("%");
    for ch in word.chars() {
        if matches!(ch, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('%');
    out
}

fn placeholders(n: usize) -> String {
    vec!["?"; n].join(",")
}

impl Db {
    pub fn query_notes(&self, q: &NoteQuery) -> Result<Vec<NoteSummary>> {
        let mut wheres: Vec<String> = vec!["n.deleted_at IS NULL".into()];
        let mut args: Vec<Value> = Vec::new();

        if let Some(nb) = &q.notebook_id {
            if q.include_descendants {
                wheres.push(
                    "n.notebook_id IN (
                         WITH RECURSIVE scope(id) AS (
                             SELECT ? UNION ALL
                             SELECT c.id FROM notebook c JOIN scope ON c.parent_id = scope.id
                             WHERE c.deleted_at IS NULL
                         ) SELECT id FROM scope)"
                        .into(),
                );
            } else {
                wheres.push("n.notebook_id = ?".into());
            }
            args.push(nb.clone().into());
        }

        if !q.status_ids.is_empty() || q.no_status {
            let mut parts = Vec::new();
            if !q.status_ids.is_empty() {
                parts.push(format!("n.status_id IN ({})", placeholders(q.status_ids.len())));
                args.extend(q.status_ids.iter().cloned().map(Value::from));
            }
            if q.no_status {
                parts.push("n.status_id IS NULL".into());
            }
            wheres.push(format!("({})", parts.join(" OR ")));
        }

        let mut tag_ids = q.tag_ids.clone();
        tag_ids.sort();
        tag_ids.dedup();
        if !tag_ids.is_empty() {
            wheres.push(format!(
                "(SELECT COUNT(DISTINCT nt.tag_id) FROM note_tag nt
                  WHERE nt.note_id = n.id AND nt.tag_id IN ({})) = ?",
                placeholders(tag_ids.len())
            ));
            args.extend(tag_ids.iter().cloned().map(Value::from));
            args.push(Value::Integer(tag_ids.len() as i64));
        }

        for word in q.search.split_whitespace() {
            wheres.push("(n.title LIKE ? ESCAPE '\\' OR n.body_md LIKE ? ESCAPE '\\')".into());
            let p = like_pattern(word);
            args.push(p.clone().into());
            args.push(p.into());
        }

        let sql = format!(
            "SELECT n.id, n.notebook_id, n.title, substr(n.body_md, 1, 200), n.status_id, n.updated_at
             FROM note n WHERE {} ORDER BY n.updated_at DESC, n.id DESC",
            wheres.join(" AND ")
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows: Vec<NoteSummary> = stmt
            .query_map(params_from_iter(args), |r| {
                Ok(NoteSummary {
                    id: r.get(0)?,
                    notebook_id: r.get(1)?,
                    title: r.get(2)?,
                    snippet: r.get(3)?,
                    status_id: r.get(4)?,
                    tag_ids: Vec::new(),
                    updated_at: r.get(5)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;

        self.attach_tags(&mut rows)?;
        Ok(rows)
    }

    /// Fill `tag_ids` for all rows with a few chunked queries (not one per note).
    fn attach_tags(&self, rows: &mut [NoteSummary]) -> Result<()> {
        let mut by_note: HashMap<String, Vec<String>> = HashMap::new();
        for chunk in rows.chunks(500) {
            let sql = format!(
                "SELECT nt.note_id, nt.tag_id FROM note_tag nt
                 JOIN tag t ON t.id = nt.tag_id
                 WHERE nt.note_id IN ({}) ORDER BY lower(t.name)",
                placeholders(chunk.len())
            );
            let mut stmt = self.conn.prepare(&sql)?;
            let links = stmt.query_map(params_from_iter(chunk.iter().map(|r| &r.id)), |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?;
            for link in links {
                let (note_id, tag_id) = link?;
                by_note.entry(note_id).or_default().push(tag_id);
            }
        }
        for row in rows {
            row.tag_ids = by_note.remove(&row.id).unwrap_or_default();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tag::DEFAULT_TAG_COLOR;

    struct Fx {
        db: Db,
        a: String,  // notebook A
        a1: String, // A / A1
        b: String,  // notebook B
    }

    /// n_* helper: create a note with title/body, optional status and tags.
    fn note(db: &Db, nb: &str, title: &str, body: &str, status: Option<&str>, tags: &[&str]) -> String {
        let n = db.create_note(nb).unwrap();
        db.update_note(&n.id, title, body).unwrap();
        db.set_note_status(&n.id, status).unwrap();
        let ids: Vec<String> = tags
            .iter()
            .map(|name| {
                db.list_tags()
                    .unwrap()
                    .into_iter()
                    .find(|t| t.name == *name)
                    .map(|t| t.id)
                    .unwrap_or_else(|| db.create_tag(name, DEFAULT_TAG_COLOR).unwrap().id)
            })
            .collect();
        db.set_note_tags(&n.id, &ids).unwrap();
        n.id
    }

    fn titles(db: &Db, q: &NoteQuery) -> Vec<String> {
        let mut t: Vec<_> = db.query_notes(q).unwrap().into_iter().map(|n| n.title).collect();
        t.sort();
        t
    }

    fn fixture() -> Fx {
        let db = Db::open_in_memory().unwrap();
        let a = db.create_notebook(None, "A").unwrap().id;
        let a1 = db.create_notebook(Some(&a), "A1").unwrap().id;
        let b = db.create_notebook(None, "B").unwrap().id;
        note(&db, &a, "n1", "alpha", Some("status-active"), &["rust", "work"]);
        note(&db, &a, "n2", "beta 100% done", Some("status-complete"), &["rust"]);
        note(&db, &a1, "n3", "我的读书笔记", Some("status-active"), &["work"]);
        note(&db, &b, "n4", "gamma_x", None, &[]);
        Fx { db, a, a1, b }
    }

    #[test]
    fn scope_direct_descendants_and_all() {
        let f = fixture();
        let q = |nb: Option<&str>, d| NoteQuery {
            notebook_id: nb.map(String::from),
            include_descendants: d,
            ..Default::default()
        };
        assert_eq!(titles(&f.db, &q(Some(&f.a), false)), ["n1", "n2"]);
        assert_eq!(titles(&f.db, &q(Some(&f.a), true)), ["n1", "n2", "n3"]);
        assert_eq!(titles(&f.db, &q(Some(&f.a1), true)), ["n3"]);
        assert_eq!(titles(&f.db, &q(Some(&f.b), true)), ["n4"]);
        assert_eq!(titles(&f.db, &q(None, false)), ["n1", "n2", "n3", "n4"]);
    }

    #[test]
    fn status_filter_is_or_and_can_include_none() {
        let f = fixture();
        let q = |ids: &[&str], none| NoteQuery {
            status_ids: ids.iter().map(|s| s.to_string()).collect(),
            no_status: none,
            ..Default::default()
        };
        assert_eq!(titles(&f.db, &q(&["status-active"], false)), ["n1", "n3"]);
        assert_eq!(
            titles(&f.db, &q(&["status-active", "status-complete"], false)),
            ["n1", "n2", "n3"]
        );
        assert_eq!(titles(&f.db, &q(&[], true)), ["n4"]);
        assert_eq!(titles(&f.db, &q(&["status-complete"], true)), ["n2", "n4"]);
    }

    #[test]
    fn tag_filter_is_and() {
        let f = fixture();
        let tag = |name: &str| f.db.list_tags().unwrap().into_iter().find(|t| t.name == name).unwrap().id;
        let q = |names: &[&str]| NoteQuery {
            tag_ids: names.iter().map(|n| tag(n)).collect(),
            ..Default::default()
        };
        assert_eq!(titles(&f.db, &q(&["rust"])), ["n1", "n2"]);
        assert_eq!(titles(&f.db, &q(&["work"])), ["n1", "n3"]);
        assert_eq!(titles(&f.db, &q(&["rust", "work"])), ["n1"]);
        // Duplicate ids must not make an AND filter unsatisfiable.
        let mut dup = q(&["rust"]);
        dup.tag_ids.push(dup.tag_ids[0].clone());
        assert_eq!(titles(&f.db, &dup), ["n1", "n2"]);
    }

    #[test]
    fn search_matches_title_body_cjk_and_is_case_insensitive() {
        let f = fixture();
        let s = |text: &str| NoteQuery { search: text.into(), ..Default::default() };
        assert_eq!(titles(&f.db, &s("ALPHA")), ["n1"]);
        assert_eq!(titles(&f.db, &s("n2")), ["n2"]); // title
        assert_eq!(titles(&f.db, &s("笔记")), ["n3"]); // inside unspaced CJK
        assert_eq!(titles(&f.db, &s("beta done")), ["n2"]); // all words, any order
        assert_eq!(titles(&f.db, &s("beta missing")), Vec::<String>::new());
    }

    #[test]
    fn search_treats_like_wildcards_literally() {
        let f = fixture();
        let s = |text: &str| NoteQuery { search: text.into(), ..Default::default() };
        assert_eq!(titles(&f.db, &s("100%")), ["n2"]);
        assert_eq!(titles(&f.db, &s("%")), ["n2"]); // not "match everything"
        assert_eq!(titles(&f.db, &s("_x")), ["n4"]); // not "any char + x"
        assert_eq!(titles(&f.db, &s("a_pha")), Vec::<String>::new());
    }

    #[test]
    fn filters_combine_and_results_carry_tags() {
        let f = fixture();
        let rust = f.db.list_tags().unwrap().into_iter().find(|t| t.name == "rust").unwrap().id;
        let q = NoteQuery {
            notebook_id: Some(f.a.clone()),
            include_descendants: true,
            status_ids: vec!["status-active".into()],
            tag_ids: vec![rust.clone()],
            search: "alp".into(),
            ..Default::default()
        };
        let got = f.db.query_notes(&q).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].title, "n1");
        assert_eq!(got[0].tag_ids.len(), 2);
    }

    #[test]
    fn deleted_notes_and_notebooks_are_excluded() {
        let f = fixture();
        f.db.delete_notebook(&f.a1).unwrap();
        assert_eq!(titles(&f.db, &NoteQuery::default()), ["n1", "n2", "n4"]);
    }
}
