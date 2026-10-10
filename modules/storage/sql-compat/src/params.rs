use crate::{Error, Result, Row};
use postgres::{Client, types::ToSql};
pub trait Params {
    fn as_params(&self) -> Vec<&(dyn ToSql + Sync)>;
}

impl Params for () {
    fn as_params(&self) -> Vec<&(dyn ToSql + Sync)> {
        Vec::new()
    }
}

impl<T, const N: usize> Params for [T; N]
where
    T: ToSql + Sync,
{
    fn as_params(&self) -> Vec<&(dyn ToSql + Sync)> {
        self.iter()
            .map(|value| value as &(dyn ToSql + Sync))
            .collect()
    }
}

impl Params for &[&(dyn ToSql + Sync)] {
    fn as_params(&self) -> Vec<&(dyn ToSql + Sync)> {
        self.to_vec()
    }
}

impl Params for Vec<&(dyn ToSql + Sync)> {
    fn as_params(&self) -> Vec<&(dyn ToSql + Sync)> {
        self.clone()
    }
}

pub(crate) fn execute(client: &mut Client, sql: &str, params: impl Params) -> Result<usize> {
    let sql = translate_placeholders(sql);
    let params = params.as_params();
    Ok(client.execute(&sql, &params)? as usize)
}

pub(crate) fn query_row<T>(
    client: &mut Client,
    sql: &str,
    params: impl Params,
    mapper: impl FnOnce(&Row) -> Result<T>,
) -> Result<T> {
    let sql = translate_placeholders(sql);
    let params = params.as_params();
    let row = client
        .query_opt(&sql, &params)?
        .ok_or(Error::QueryReturnedNoRows)?;
    mapper(&Row(row))
}

pub(crate) fn translate_placeholders(sql: &str) -> String {
    let mut output = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    let mut in_string = false;
    let mut next_implicit = 1usize;
    while let Some(ch) = chars.next() {
        if ch == '\'' {
            output.push(ch);
            if in_string && chars.peek() == Some(&'\'') {
                output.push(chars.next().expect("peeked quote"));
            } else {
                in_string = !in_string;
            }
            continue;
        }
        if ch != '?' || in_string {
            output.push(ch);
            continue;
        }
        let mut digits = String::new();
        while chars.peek().is_some_and(|next| next.is_ascii_digit()) {
            digits.push(chars.next().expect("peeked digit"));
        }
        let index = if digits.is_empty() {
            let index = next_implicit;
            next_implicit += 1;
            index
        } else {
            let index = digits.parse::<usize>().expect("ASCII digits");
            next_implicit = next_implicit.max(index + 1);
            index
        };
        output.push('$');
        output.push_str(&index.to_string());
    }
    output
}

#[macro_export]
macro_rules! params {
    () => {
        Vec::<&(dyn $crate::types::ToSql + Sync)>::new()
    };
    ($($value:expr),+ $(,)?) => {
        vec![$(&$value as &(dyn $crate::types::ToSql + Sync)),+]
    };
}
