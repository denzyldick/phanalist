#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FindingClass {
    Division,
    Modulo,
    Intdiv,
    IntdivOverflow,
    ArrayKey,
    ArrayIndex,
    StringOffset,
    NullMethod,
    NullProperty,
    ReturnContract,
    Unreachable,
    Assertion,
}

impl FindingClass {
    pub const ALL: [Self; 12] = [
        Self::Division,
        Self::Modulo,
        Self::Intdiv,
        Self::IntdivOverflow,
        Self::ArrayKey,
        Self::ArrayIndex,
        Self::StringOffset,
        Self::NullMethod,
        Self::NullProperty,
        Self::ReturnContract,
        Self::Unreachable,
        Self::Assertion,
    ];

    pub const fn code(self) -> &'static str {
        match self {
            Self::Division => "E0031",
            Self::Modulo => "E0032",
            Self::Intdiv => "E0033",
            Self::IntdivOverflow => "E0034",
            Self::ArrayKey => "E0035",
            Self::ArrayIndex => "E0036",
            Self::StringOffset => "E0037",
            Self::NullMethod => "E0038",
            Self::NullProperty => "E0039",
            Self::ReturnContract => "E0040",
            Self::Unreachable => "E0041",
            Self::Assertion => "E0042",
        }
    }

    #[allow(dead_code)]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Division => "division-by-zero",
            Self::Modulo => "modulo-by-zero",
            Self::Intdiv => "intdiv-by-zero",
            Self::IntdivOverflow => "intdiv-overflow",
            Self::ArrayKey => "array-key",
            Self::ArrayIndex => "array-index",
            Self::StringOffset => "string-offset",
            Self::NullMethod => "null-method-call",
            Self::NullProperty => "null-property-access",
            Self::ReturnContract => "return-contract",
            Self::Unreachable => "unreachable",
            Self::Assertion => "assertion",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Division => "Division by zero",
            Self::Modulo => "Modulo by zero",
            Self::Intdiv => "intdiv() by zero",
            Self::IntdivOverflow => "intdiv() overflow",
            Self::ArrayKey => "Missing array key",
            Self::ArrayIndex => "Array index out of bounds",
            Self::StringOffset => "String offset out of bounds",
            Self::NullMethod => "Method call on null",
            Self::NullProperty => "Property access on null",
            Self::ReturnContract => "Return contract violation",
            Self::Unreachable => "Unreachable code",
            Self::Assertion => "Impossible assertion",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|class| class.code() == code)
    }

    pub fn message_id(self, id: &str) -> String {
        format!("{}:{id}", self.code())
    }
}
