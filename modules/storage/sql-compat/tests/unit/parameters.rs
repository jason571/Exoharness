use crate::params::translate_placeholders;
#[test]
fn placeholders_skip_string_literals() {
    assert_eq!(
        translate_placeholders("a=?1 AND literal='?2' AND b=?"),
        "a=$1 AND literal='?2' AND b=$2"
    );
}
