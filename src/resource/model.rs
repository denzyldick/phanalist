#![allow(clippy::should_implement_trait)]

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SizeExpr {
    Unknown,
    Const(u64),
    Param(String),
    CountOf(String),
    BytesOf(String),
    FileSize(String),
    Stream(String),
    Sum(Vec<SizeExpr>),
    Product(Box<SizeExpr>, u64),
    Mul(Box<SizeExpr>, Box<SizeExpr>),
    Min(Box<SizeExpr>, Box<SizeExpr>),
    Max(Box<SizeExpr>, Box<SizeExpr>),
}

impl SizeExpr {
    pub fn count_of(name: impl Into<String>) -> Self {
        Self::CountOf(name.into())
    }

    pub fn bytes_of(name: impl Into<String>) -> Self {
        Self::BytesOf(name.into())
    }

    pub fn file_size(name: impl Into<String>) -> Self {
        Self::FileSize(name.into())
    }

    pub fn stream(name: impl Into<String>) -> Self {
        Self::Stream(name.into())
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }

    pub fn as_const(&self) -> Option<u64> {
        match self {
            Self::Const(value) => Some(*value),
            _ => None,
        }
    }

    pub fn is_bounded(&self) -> bool {
        match self {
            Self::Const(_) => true,
            Self::Param(_)
            | Self::CountOf(_)
            | Self::BytesOf(_)
            | Self::FileSize(_)
            | Self::Stream(_)
            | Self::Unknown => false,
            Self::Sum(values) => values.iter().all(Self::is_bounded),
            Self::Product(value, _) => value.is_bounded(),
            Self::Mul(left, right) => left.is_bounded() && right.is_bounded(),
            Self::Min(left, right) => left.is_bounded() || right.is_bounded(),
            Self::Max(left, right) => left.is_bounded() && right.is_bounded(),
        }
    }

    pub fn has_unknown(&self) -> bool {
        match self {
            Self::Unknown => true,
            Self::Const(_)
            | Self::Param(_)
            | Self::CountOf(_)
            | Self::BytesOf(_)
            | Self::FileSize(_)
            | Self::Stream(_) => false,
            Self::Sum(values) => values.iter().any(Self::has_unknown),
            Self::Product(value, _) => value.has_unknown(),
            Self::Mul(left, right) => left.has_unknown() || right.has_unknown(),
            Self::Min(left, right) | Self::Max(left, right) => {
                left.has_unknown() || right.has_unknown()
            }
        }
    }

    pub fn contains_param(&self, name: &str) -> bool {
        match self {
            Self::Param(value)
            | Self::CountOf(value)
            | Self::BytesOf(value)
            | Self::FileSize(value) => value == name,
            Self::Sum(values) => values.iter().any(|value| value.contains_param(name)),
            Self::Product(value, _) => value.contains_param(name),
            Self::Mul(left, right) => left.contains_param(name) || right.contains_param(name),
            Self::Min(left, right) | Self::Max(left, right) => {
                left.contains_param(name) || right.contains_param(name)
            }
            Self::Unknown | Self::Const(_) | Self::Stream(_) => false,
        }
    }

    pub fn add(self, other: Self) -> Self {
        match (self, other) {
            (Self::Const(left), Self::Const(right)) => Self::Const(left.saturating_add(right)),
            (Self::Const(0), value) | (value, Self::Const(0)) => value,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (left, right) => {
                let mut values = Vec::new();
                match left {
                    Self::Sum(inner) => values.extend(inner),
                    value => values.push(value),
                }
                match right {
                    Self::Sum(inner) => values.extend(inner),
                    value => values.push(value),
                }
                Self::Sum(values)
            }
        }
    }

    pub fn mul(self, other: Self) -> Self {
        match (self, other) {
            (Self::Const(left), Self::Const(right)) => Self::Const(left.saturating_mul(right)),
            (Self::Const(0), _) | (_, Self::Const(0)) => Self::Const(0),
            (Self::Const(1), value) | (value, Self::Const(1)) => value,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (left, right) => Self::Mul(Box::new(left), Box::new(right)),
        }
    }

    pub fn mul_const(self, multiplier: u64) -> Self {
        if multiplier == 0 {
            return Self::Const(0);
        }
        if multiplier == 1 {
            return self;
        }
        match self {
            Self::Const(value) => Self::Const(value.saturating_mul(multiplier)),
            Self::Product(value, existing) => {
                Self::Product(value, existing.saturating_mul(multiplier))
            }
            Self::Unknown => Self::Unknown,
            value => Self::Product(Box::new(value), multiplier),
        }
    }

    pub fn min(self, other: Self) -> Self {
        match (self, other) {
            (left, right) if left == right => left,
            (Self::Const(left), Self::Const(right)) => Self::Const(left.min(right)),
            (left, right) => Self::Min(Box::new(left), Box::new(right)),
        }
    }

    pub fn max(self, other: Self) -> Self {
        match (self, other) {
            (Self::Const(left), Self::Const(right)) => Self::Const(left.max(right)),
            (Self::Const(0), value) | (value, Self::Const(0)) => value,
            (left, right) => {
                let mut values = Vec::new();
                flatten_max(left, &mut values);
                flatten_max(right, &mut values);

                let mut seen = HashSet::new();
                values.retain(|value| seen.insert(value.clone()));
                build_max(values)
            }
        }
    }

    pub fn substitute(&self, replacements: &BTreeMap<String, SizeExpr>) -> Self {
        match self {
            Self::Param(name)
            | Self::CountOf(name)
            | Self::BytesOf(name)
            | Self::FileSize(name) => replacements
                .get(name)
                .cloned()
                .unwrap_or_else(|| self.clone()),
            Self::Unknown => Self::Unknown,
            Self::Const(value) => Self::Const(*value),
            Self::Stream(name) => Self::Stream(name.clone()),
            Self::Sum(values) => values.iter().fold(Self::Const(0), |acc, value| {
                acc.add(value.substitute(replacements))
            }),
            Self::Product(value, multiplier) => {
                value.substitute(replacements).mul_const(*multiplier)
            }
            Self::Mul(left, right) => left
                .substitute(replacements)
                .mul(right.substitute(replacements)),
            Self::Min(left, right) => left
                .substitute(replacements)
                .min(right.substitute(replacements)),
            Self::Max(left, right) => left
                .substitute(replacements)
                .max(right.substitute(replacements)),
        }
    }
}

fn flatten_max(value: SizeExpr, values: &mut Vec<SizeExpr>) {
    match value {
        SizeExpr::Max(left, right) => {
            flatten_max(*left, values);
            flatten_max(*right, values);
        }
        value => values.push(value),
    }
}

fn build_max(mut values: Vec<SizeExpr>) -> SizeExpr {
    if values.is_empty() {
        return SizeExpr::Const(0);
    }

    while values.len() > 1 {
        let mut next = Vec::with_capacity(values.len().div_ceil(2));
        let mut iter = values.into_iter();
        while let Some(left) = iter.next() {
            match iter.next() {
                Some(right) => next.push(SizeExpr::Max(Box::new(left), Box::new(right))),
                None => next.push(left),
            }
        }
        values = next;
    }

    values.pop().unwrap_or(SizeExpr::Const(0))
}

impl fmt::Display for SizeExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown => write!(f, "?"),
            Self::Const(value) => write!(f, "{value}"),
            Self::Param(name) | Self::CountOf(name) => write!(f, "n({name})"),
            Self::BytesOf(name) => write!(f, "bytes({name})"),
            Self::FileSize(name) => write!(f, "filesize({name})"),
            Self::Stream(name) => write!(f, "stream({name})"),
            Self::Sum(values) => {
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        write!(f, " + ")?;
                    }
                    write!(f, "{value}")?;
                }
                Ok(())
            }
            Self::Product(value, multiplier) => write!(f, "{value} * {multiplier}"),
            Self::Mul(left, right) => write!(f, "{left} * {right}"),
            Self::Min(left, right) => {
                write!(f, "min(")?;
                write_chain(f, left, "min")?;
                write!(f, ", ")?;
                write_chain(f, right, "min")?;
                write!(f, ")")
            }
            Self::Max(left, right) => {
                write!(f, "max(")?;
                write_chain(f, left, "max")?;
                write!(f, ", ")?;
                write_chain(f, right, "max")?;
                write!(f, ")")
            }
        }
    }
}

fn write_chain(f: &mut fmt::Formatter<'_>, value: &SizeExpr, operation: &str) -> fmt::Result {
    match (operation, value) {
        ("min", SizeExpr::Min(left, right)) => {
            write_chain(f, left, operation)?;
            write!(f, ", ")?;
            write_chain(f, right, operation)
        }
        ("max", SizeExpr::Max(left, right)) => {
            write_chain(f, left, operation)?;
            write!(f, ", ")?;
            write_chain(f, right, operation)
        }
        _ => write!(f, "{value}"),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum CostExpr {
    #[default]
    Constant,
    Polynomial(Vec<SizeExpr>),
    Exponential(SizeExpr),
    Unknown,
}

impl CostExpr {
    pub fn linear(size: SizeExpr) -> Self {
        if size.is_bounded() {
            return Self::Constant;
        }
        if size.has_unknown() {
            return Self::Unknown;
        }
        Self::Polynomial(vec![size])
    }

    pub fn quadratic(size: SizeExpr) -> Self {
        Self::linear(size.clone()).multiply_by(size)
    }

    pub fn multiply_by(self, size: SizeExpr) -> Self {
        if size.is_bounded() {
            return self;
        }
        if size.has_unknown() {
            return Self::Unknown;
        }
        match self {
            Self::Constant => Self::linear(size),
            Self::Polynomial(mut factors) => {
                if factors.len() < 8 {
                    factors.push(size);
                }
                Self::Polynomial(factors)
            }
            Self::Exponential(base) => Self::Exponential(base.max(size)),
            Self::Unknown => Self::Unknown,
        }
    }

    pub fn add(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unknown, value) | (value, Self::Unknown) => value,
            (Self::Constant, value) | (value, Self::Constant) => value,
            (Self::Exponential(base), _) | (_, Self::Exponential(base)) => Self::Exponential(base),
            (Self::Polynomial(mut left), Self::Polynomial(right)) => {
                if right.len() > left.len() {
                    left = right;
                }
                Self::Polynomial(left)
            }
        }
    }

    pub fn degree(&self) -> u8 {
        match self {
            Self::Constant => 0,
            Self::Polynomial(factors) => u8::try_from(factors.len()).unwrap_or(u8::MAX),
            Self::Exponential(_) => u8::MAX,
            Self::Unknown => 0,
        }
    }

    pub fn is_bounded(&self) -> bool {
        match self {
            Self::Constant => true,
            Self::Polynomial(factors) => factors.iter().all(SizeExpr::is_bounded),
            Self::Exponential(_) | Self::Unknown => false,
        }
    }

    pub fn substitute(&self, replacements: &BTreeMap<String, SizeExpr>) -> Self {
        match self {
            Self::Constant => Self::Constant,
            Self::Unknown => Self::Unknown,
            Self::Polynomial(factors) => {
                let mut result = Self::Constant;
                for factor in factors {
                    result = result.multiply_by(factor.substitute(replacements));
                }
                result
            }
            Self::Exponential(base) => Self::Exponential(base.substitute(replacements)),
        }
    }
}

impl fmt::Display for CostExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Constant => write!(f, "O(1)"),
            Self::Unknown => write!(f, "O(?)"),
            Self::Exponential(base) => write!(f, "O(2^{base})"),
            Self::Polynomial(factors) => {
                if factors.len() == 2 && factors[0] == factors[1] {
                    return write!(f, "O({}^2)", factors[0]);
                }
                write!(f, "O(")?;
                for (index, factor) in factors.iter().enumerate() {
                    if index > 0 {
                        write!(f, " * ")?;
                    }
                    write!(f, "{factor}")?;
                }
                write!(f, ")")
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeapEffect {
    pub alloc: CostExpr,
    pub retained: SizeExpr,
    pub peak: SizeExpr,
    pub tainted: bool,
}

impl Default for HeapEffect {
    fn default() -> Self {
        Self {
            alloc: CostExpr::Constant,
            retained: SizeExpr::Const(0),
            peak: SizeExpr::Const(0),
            tainted: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrowthKind {
    Append,
    Retain,
    Copy,
    Unshift,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Growth {
    pub kind: GrowthKind,
    pub count_per_iteration: SizeExpr,
    pub bytes_per_iteration: SizeExpr,
}

impl Growth {
    pub fn append(bytes_per_iteration: SizeExpr) -> Self {
        Self {
            kind: GrowthKind::Append,
            count_per_iteration: SizeExpr::Const(1),
            bytes_per_iteration,
        }
    }

    pub fn retain(bytes_per_iteration: SizeExpr) -> Self {
        Self {
            kind: GrowthKind::Retain,
            count_per_iteration: SizeExpr::Const(0),
            bytes_per_iteration,
        }
    }

    pub fn copy(count_per_iteration: SizeExpr, bytes_per_iteration: SizeExpr) -> Self {
        Self {
            kind: GrowthKind::Copy,
            count_per_iteration,
            bytes_per_iteration,
        }
    }

    pub fn unshift(bytes_per_iteration: SizeExpr) -> Self {
        Self {
            kind: GrowthKind::Unshift,
            count_per_iteration: SizeExpr::Const(1),
            bytes_per_iteration,
        }
    }

    pub fn is_copying(&self) -> bool {
        matches!(self.kind, GrowthKind::Copy | GrowthKind::Unshift)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueState {
    pub count: SizeExpr,
    pub bytes: SizeExpr,
    pub element_bytes: SizeExpr,
    pub alloc: CostExpr,
    pub peak: SizeExpr,
    pub tainted: bool,
    pub taint_origins: BTreeSet<String>,
    pub stream: bool,
    pub scalar: bool,
    pub growth: Option<Growth>,
}

impl Default for ValueState {
    fn default() -> Self {
        Self {
            count: SizeExpr::Unknown,
            bytes: SizeExpr::Unknown,
            element_bytes: SizeExpr::Unknown,
            alloc: CostExpr::Constant,
            peak: SizeExpr::Const(0),
            tainted: false,
            taint_origins: BTreeSet::new(),
            stream: false,
            scalar: false,
            growth: None,
        }
    }
}

impl ValueState {
    pub fn scalar() -> Self {
        Self {
            count: SizeExpr::Const(1),
            bytes: SizeExpr::Const(8),
            element_bytes: SizeExpr::Const(8),
            alloc: CostExpr::Constant,
            peak: SizeExpr::Const(8),
            tainted: false,
            taint_origins: BTreeSet::new(),
            stream: false,
            scalar: true,
            growth: None,
        }
    }

    pub fn string(bytes: SizeExpr, tainted: bool) -> Self {
        Self {
            count: bytes.clone(),
            bytes: bytes.clone(),
            element_bytes: SizeExpr::Const(1),
            alloc: CostExpr::linear(bytes.clone()),
            peak: bytes,
            tainted,
            taint_origins: BTreeSet::new(),
            stream: false,
            scalar: true,
            growth: None,
        }
    }

    pub fn array(count: SizeExpr, bytes: SizeExpr, element_bytes: SizeExpr, tainted: bool) -> Self {
        Self {
            count,
            bytes: bytes.clone(),
            element_bytes,
            alloc: CostExpr::linear(bytes.clone()),
            peak: bytes,
            tainted,
            taint_origins: BTreeSet::new(),
            stream: false,
            scalar: false,
            growth: None,
        }
    }

    pub fn param(name: impl Into<String>) -> Self {
        let name = name.into();
        let mut taint_origins = BTreeSet::new();
        taint_origins.insert(name.clone());
        Self {
            count: SizeExpr::count_of(name.clone()),
            bytes: SizeExpr::bytes_of(name.clone()),
            element_bytes: SizeExpr::Unknown,
            alloc: CostExpr::Constant,
            peak: SizeExpr::Const(0),
            tainted: false,
            taint_origins,
            stream: false,
            scalar: false,
            growth: None,
        }
    }

    pub fn merge(left: &Self, right: &Self) -> Self {
        if left == right {
            return left.clone();
        }
        Self {
            count: left.count.clone().max(right.count.clone()),
            bytes: left.bytes.clone().max(right.bytes.clone()),
            element_bytes: left.element_bytes.clone().max(right.element_bytes.clone()),
            alloc: left.alloc.clone().add(right.alloc.clone()),
            peak: left.peak.clone().max(right.peak.clone()),
            tainted: left.tainted || right.tainted,
            taint_origins: left
                .taint_origins
                .union(&right.taint_origins)
                .cloned()
                .collect(),
            stream: left.stream && right.stream,
            scalar: left.scalar && right.scalar,
            growth: left.growth.clone().or_else(|| right.growth.clone()),
        }
    }
}
