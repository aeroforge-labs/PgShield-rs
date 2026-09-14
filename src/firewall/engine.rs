// --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                #*eddiere
//! Firewall Engine that parses SQL into AST and evaluates active rules

use crate::config::Config;
use crate::firewall::rules::{
    BlockDDLRule, FirewallRule, RequireLimitRule, RequireWhereRule, RuleViolation,
};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use tracing::{debug, warn};

pub struct QueryFirewall {
    rules: Vec<Box<dyn FirewallRule>>,
    strict_mode: bool,
}

impl QueryFirewall {
    pub fn new(config: &Config) -> Self {
        let mut rules: Vec<Box<dyn FirewallRule>> = Vec::new();

        if config.require_where {
            rules.push(Box::new(RequireWhereRule));
        }

        if config.require_limit {
            rules.push(Box::new(RequireLimitRule));
        }

        if config.block_ddl {
            rules.push(Box::new(BlockDDLRule));
        }

        Self {
            rules,
            strict_mode: config.strict_firewall,
        }
    }

    pub fn inspect_statement(&self, raw_sql: &str) -> Result<(), RuleViolation> {
        let dialect = PostgreSqlDialect {};

        let ast = match Parser::parse_sql(&dialect, raw_sql) {
            Ok(statements) => statements,
            Err(e) => {
                debug!("SQL AST parse error: {}, passing statement through", e);
                return Ok(());
            }
        };

        for statement in &ast {
            for rule in &self.rules {
                if let Err(violation) = rule.evaluate(statement) {
                    warn!(
                        rule = rule.name(),
                        violation = %violation,
                        sql = %raw_sql,
                        "Firewall rule violation detected"
                    );

                    if self.strict_mode {
                        return Err(violation);
                    }
                }
            }
        }

        Ok(())
    }
}
