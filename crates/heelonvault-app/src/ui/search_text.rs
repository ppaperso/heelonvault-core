//! Text normalization shared by the search index and the editor's
//! category/tag suggestions, so both agree on what "the same word" is.

/// Lower-case, accent-folded, punctuation-collapsed form used for matching and
/// de-duplication (`"Équipe-Prod"` → `"equipe prod"`).
pub(crate) fn normalize_search_text(raw: &str) -> String {
    let mut normalized = String::with_capacity(raw.len());
    for ch in raw.chars() {
        let mapped = match ch {
            'a'..='z' | '0'..='9' => ch,
            'A'..='Z' => ch.to_ascii_lowercase(),
            'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => {
                'a'
            }
            'Ç' | 'ç' => 'c',
            'È' | 'É' | 'Ê' | 'Ë' | 'è' | 'é' | 'ê' | 'ë' => 'e',
            'Ì' | 'Í' | 'Î' | 'Ï' | 'ì' | 'í' | 'î' | 'ï' => 'i',
            'Ñ' | 'ñ' => 'n',
            'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => {
                'o'
            }
            'Ù' | 'Ú' | 'Û' | 'Ü' | 'ù' | 'ú' | 'û' | 'ü' => 'u',
            'Ý' | 'Ÿ' | 'ý' | 'ÿ' => 'y',
            'Æ' | 'æ' => 'a',
            'Œ' | 'œ' => 'o',
            _ => {
                if ch.is_whitespace() || ch == '-' || ch == '_' || ch == '.' || ch == '@' {
                    ' '
                } else {
                    continue;
                }
            }
        };
        normalized.push(mapped);
    }
    normalized
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}
