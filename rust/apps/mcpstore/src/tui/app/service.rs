#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddServiceMode {
    Stdio,
    Http,
    Json,
    Toml,
}

impl AddServiceMode {
    pub const ALL: [Self; 4] = [Self::Stdio, Self::Http, Self::Json, Self::Toml];
    pub const MENU: [Self; 4] = [Self::Http, Self::Stdio, Self::Json, Self::Toml];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Stdio => "stdio",
            Self::Http => "http",
            Self::Json => "json",
            Self::Toml => "toml",
        }
    }

    pub fn menu_label(&self) -> &'static str {
        match self {
            Self::Http => "添加http服务",
            Self::Stdio => "添加stdio服务",
            Self::Json => "从json添加",
            Self::Toml => "从toml添加",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddServiceField {
    Name,
    Command,
    Args,
    Url,
    Description,
    WorkingDir,
    Env,
    Headers,
    Scope,
    Agent,
    ConnectAfterAdd,
    Json,
    Toml,
    Submit,
}

impl AddServiceField {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Command => "Command",
            Self::Args => "Args",
            Self::Url => "URL",
            Self::Description => "Description",
            Self::WorkingDir => "Working directory",
            Self::Env => "Env vars",
            Self::Headers => "Headers",
            Self::Scope => "Scope",
            Self::Agent => "Agent ID",
            Self::ConnectAfterAdd => "Connect after add",
            Self::Json => "ServerConfig JSON",
            Self::Toml => "ServerConfig TOML",
            Self::Submit => "Add service",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddServicePane {
    Menu,
    Form,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddServiceSection {
    Basic,
    Connection,
    Scope,
    Advanced,
    Submit,
}

impl AddServiceSection {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Basic => "基础信息",
            Self::Connection => "连接配置",
            Self::Scope => "作用域",
            Self::Advanced => "高级配置",
            Self::Submit => "提交",
        }
    }
}

#[derive(Clone, Debug)]
pub struct AddServiceFormState {
    pub mode: AddServiceMode,
    pub pane: AddServicePane,
    pub selected_section: usize,
    pub selected_field: usize,
    pub name: String,
    pub command: String,
    pub args: String,
    pub url: String,
    pub description: String,
    pub working_dir: String,
    pub env: String,
    pub headers: String,
    pub scope: String,
    pub agent: String,
    pub connect_after_add: String,
    pub json: String,
    pub toml: String,
}

impl Default for AddServiceFormState {
    fn default() -> Self {
        Self {
            mode: AddServiceMode::Http,
            pane: AddServicePane::Menu,
            selected_section: 0,
            selected_field: 0,
            name: String::new(),
            command: "npx -y @modelcontextprotocol/server-filesystem .".to_string(),
            args: String::new(),
            url: "http://127.0.0.1:8000/mcp".to_string(),
            description: String::new(),
            working_dir: String::new(),
            env: String::new(),
            headers: String::new(),
            scope: "store".to_string(),
            agent: String::new(),
            connect_after_add: "yes".to_string(),
            json: "{ \"command\": \"npx\", \"args\": [\"-y\", \"@modelcontextprotocol/server-filesystem\", \".\"], \"transport\": \"stdio\" }".to_string(),
            toml: "command = \"npx\"\nargs = [\"-y\", \"@modelcontextprotocol/server-filesystem\", \".\"]\ntransport = \"stdio\"".to_string(),
        }
    }
}

impl AddServiceFormState {
    pub fn fields(&self) -> &'static [AddServiceField] {
        match self.mode {
            AddServiceMode::Stdio => &[
                AddServiceField::Name,
                AddServiceField::Description,
                AddServiceField::Command,
                AddServiceField::Args,
                AddServiceField::WorkingDir,
                AddServiceField::Env,
                AddServiceField::Scope,
                AddServiceField::Agent,
                AddServiceField::ConnectAfterAdd,
                AddServiceField::Submit,
            ],
            AddServiceMode::Http => &[
                AddServiceField::Name,
                AddServiceField::Description,
                AddServiceField::Url,
                AddServiceField::Headers,
                AddServiceField::Scope,
                AddServiceField::Agent,
                AddServiceField::ConnectAfterAdd,
                AddServiceField::Submit,
            ],
            AddServiceMode::Json => &[
                AddServiceField::Name,
                AddServiceField::Json,
                AddServiceField::Scope,
                AddServiceField::Agent,
                AddServiceField::ConnectAfterAdd,
                AddServiceField::Submit,
            ],
            AddServiceMode::Toml => &[
                AddServiceField::Name,
                AddServiceField::Toml,
                AddServiceField::Scope,
                AddServiceField::Agent,
                AddServiceField::ConnectAfterAdd,
                AddServiceField::Submit,
            ],
        }
    }

    pub fn sections(&self) -> &'static [AddServiceSection] {
        match self.mode {
            AddServiceMode::Stdio => &[
                AddServiceSection::Basic,
                AddServiceSection::Connection,
                AddServiceSection::Scope,
                AddServiceSection::Advanced,
                AddServiceSection::Submit,
            ],
            AddServiceMode::Http => &[
                AddServiceSection::Basic,
                AddServiceSection::Connection,
                AddServiceSection::Scope,
                AddServiceSection::Submit,
            ],
            AddServiceMode::Json | AddServiceMode::Toml => &[
                AddServiceSection::Basic,
                AddServiceSection::Connection,
                AddServiceSection::Scope,
                AddServiceSection::Submit,
            ],
        }
    }

    pub fn selected_section(&self) -> AddServiceSection {
        self.sections()
            .get(self.selected_section)
            .copied()
            .unwrap_or(AddServiceSection::Basic)
    }

    pub fn fields_for_section(&self, section: AddServiceSection) -> &'static [AddServiceField] {
        match (self.mode, section) {
            (AddServiceMode::Stdio | AddServiceMode::Http, AddServiceSection::Basic) => {
                &[AddServiceField::Name, AddServiceField::Description]
            }
            (AddServiceMode::Json | AddServiceMode::Toml, AddServiceSection::Basic) => {
                &[AddServiceField::Name]
            }
            (AddServiceMode::Stdio, AddServiceSection::Connection) => {
                &[AddServiceField::Command, AddServiceField::Args]
            }
            (AddServiceMode::Http, AddServiceSection::Connection) => {
                &[AddServiceField::Url, AddServiceField::Headers]
            }
            (AddServiceMode::Json, AddServiceSection::Connection) => &[AddServiceField::Json],
            (AddServiceMode::Toml, AddServiceSection::Connection) => &[AddServiceField::Toml],
            (_, AddServiceSection::Scope) => &[
                AddServiceField::Scope,
                AddServiceField::Agent,
                AddServiceField::ConnectAfterAdd,
            ],
            (AddServiceMode::Stdio, AddServiceSection::Advanced) => {
                &[AddServiceField::WorkingDir, AddServiceField::Env]
            }
            (AddServiceMode::Http, AddServiceSection::Advanced) => &[],
            (_, AddServiceSection::Advanced) => &[],
            (_, AddServiceSection::Submit) => &[AddServiceField::Submit],
        }
    }

    pub fn selected_fields(&self) -> &'static [AddServiceField] {
        self.fields()
    }

    pub fn selected_field(&self) -> AddServiceField {
        self.selected_fields()
            .get(self.selected_field)
            .copied()
            .unwrap_or(AddServiceField::Name)
    }
}
