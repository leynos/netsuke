use camino::{Utf8Path, Utf8PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct TemplatePath(pub(crate) Utf8PathBuf);

impl TemplatePath {
    pub(crate) fn as_path(&self) -> &Utf8Path {
        &self.0
    }
}

impl From<String> for TemplatePath {
    fn from(value: String) -> Self {
        Self(Utf8PathBuf::from(value))
    }
}

impl From<Utf8PathBuf> for TemplatePath {
    fn from(value: Utf8PathBuf) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TemplateContent(String);

impl TemplateContent {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for TemplateContent {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FileContent(String);

impl FileContent {
    pub(crate) const fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl From<String> for FileContent {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RelativePath(String);

impl RelativePath {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn to_path_buf(&self) -> Utf8PathBuf {
        Utf8PathBuf::from(self.as_str())
    }
}

impl From<String> for RelativePath {
    fn from(value: String) -> Self {
        Self(value)
    }
}
