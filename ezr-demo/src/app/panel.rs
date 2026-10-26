#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Panel {
    #[default]
    List,

    Detail,

    Conns,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DetailField {
    Checksum,

    Url,

    Failed,

    SavePath,
}

impl DetailField {
    pub fn title(self) -> &'static str {
        match self {
            DetailField::Checksum => "校验",
            DetailField::Url => "URL",
            DetailField::Failed => "失败",
            DetailField::SavePath => "路径",
        }
    }

    pub fn copy_label(self) -> &'static str {
        match self {
            DetailField::Checksum => "校验码",
            DetailField::Url => "url",
            DetailField::Failed => "失败原因",
            DetailField::SavePath => "保存路径",
        }
    }
}

impl Panel {
    pub fn next(self) -> Self {
        match self {
            Panel::List => Panel::Detail,
            Panel::Detail => Panel::Conns,
            Panel::Conns => Panel::List,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Panel::List => Panel::Conns,
            Panel::Detail => Panel::List,
            Panel::Conns => Panel::Detail,
        }
    }
}
