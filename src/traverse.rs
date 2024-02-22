use php_parser_rs::parser::ast::{classes::ClassMember, functions::MethodBody, Statement};

pub struct Traverse {
    statement: Statement,
}

impl Traverse {
    pub fn new(statement: Statement) -> Self {
        Self { statement }
    }
    pub fn find_class_members(self, incl_constructor: bool, r: Validate) {
        if let Statement::Class(class) = self.statement {
            for member in &class.body.members {
                if let ClassMember::ConcreteMethod(concretemethod) = member {
                    let MethodBody {
                        comments: _,
                        left_brace: _,
                        statements,
                        right_brace: _,
                    } = &concretemethod.body;
                    {
                        r();
                    }
                }
            }
        }
    }
}

type Validate = fn();
