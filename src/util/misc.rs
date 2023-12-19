#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MemberNameType {
    Private,
    Quoted,
    Normal,
    Expression,
}
