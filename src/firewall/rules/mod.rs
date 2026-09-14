// --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                #*eddiere
//! Firewall Rule Engine Rules

use sqlparser::ast::Statement;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum RuleViolation {
    #[error("UPDATE or DELETE statement missing WHERE clause selection")]
    MissingWhereClause,

    #[error("SELECT statement missing LIMIT clause")]
    MissingLimitClause,

    #[error("Destructive DDL statement ({0}) prohibited in strict mode")]
    ForbiddenDDL(String),
}

pub trait FirewallRule: Send + Sync {
    fn name(&self) -> &'static str;
    fn evaluate(&self, statement: &Statement) -> Result<(), RuleViolation>;
}

pub struct RequireWhereRule;

impl FirewallRule for RequireWhereRule {
    fn name(&self) -> &'static str {
        "RequireWhereRule"
    }

    fn evaluate(&self, statement: &Statement) -> Result<(), RuleViolation> {
        match statement {
            Statement::Update { selection, .. } => {
                if selection.is_none() {
                    return Err(RuleViolation::MissingWhereClause);
                }
            }
            Statement::Delete(delete) => {
                if delete.selection.is_none() {
                    return Err(RuleViolation::MissingWhereClause);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

pub struct RequireLimitRule;

impl FirewallRule for RequireLimitRule {
    fn name(&self) -> &'static str {
        "RequireLimitRule"
    }

    fn evaluate(&self, statement: &Statement) -> Result<(), RuleViolation> {
        if let Statement::Query(query) = statement {
            if query.limit.is_none() {
                return Err(RuleViolation::MissingLimitClause);
            }
        }
        Ok(())
    }
}

pub struct BlockDDLRule;

impl FirewallRule for BlockDDLRule {
    fn name(&self) -> &'static str {
        "BlockDDLRule"
    }

    fn evaluate(&self, statement: &Statement) -> Result<(), RuleViolation> {
        match statement {
            Statement::Drop { object_type, .. } => Err(RuleViolation::ForbiddenDDL(format!(
                "DROP {:?}",
                object_type
            ))),
            Statement::Truncate { .. } => Err(RuleViolation::ForbiddenDDL("TRUNCATE".to_string())),
            Statement::AlterTable { .. } => {
                Err(RuleViolation::ForbiddenDDL("ALTER TABLE".to_string()))
            }
            _ => Ok(()),
        }
    }
}
