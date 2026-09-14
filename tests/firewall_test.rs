// --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                #*eddiere
use pgshield::config::Config;
use pgshield::firewall::{QueryFirewall, RuleViolation};

fn create_test_firewall(require_where: bool, require_limit: bool, block_ddl: bool) -> QueryFirewall {
    let config = Config {
        listen_addr: "127.0.0.1:6432".parse().unwrap(),
        backend_host: "127.0.0.1".to_string(),
        backend_port: 5432,
        pool_max_size: 10,
        strict_firewall: true,
        require_where,
        require_limit,
        block_ddl,
    };
    QueryFirewall::new(&config)
}

#[test]
fn test_require_where_blocks_unbounded_update() {
    let fw = create_test_firewall(true, false, false);
    
    // Unbounded UPDATE without WHERE should be blocked
    let res = fw.inspect_statement("UPDATE users SET active = false;");
    assert_eq!(res, Err(RuleViolation::MissingWhereClause));

    // UPDATE with WHERE should pass
    let res_ok = fw.inspect_statement("UPDATE users SET active = false WHERE id = 42;");
    assert!(res_ok.is_ok());
}

#[test]
fn test_require_where_blocks_unbounded_delete() {
    let fw = create_test_firewall(true, false, false);

    // Unbounded DELETE without WHERE should be blocked
    let res = fw.inspect_statement("DELETE FROM audit_logs;");
    assert_eq!(res, Err(RuleViolation::MissingWhereClause));

    // DELETE with WHERE should pass
    let res_ok = fw.inspect_statement("DELETE FROM audit_logs WHERE created_at < NOW();");
    assert!(res_ok.is_ok());
}

#[test]
fn test_require_limit_blocks_unbounded_select() {
    let fw = create_test_firewall(false, true, false);

    // SELECT without LIMIT should be blocked
    let res = fw.inspect_statement("SELECT * FROM large_table;");
    assert_eq!(res, Err(RuleViolation::MissingLimitClause));

    // SELECT with LIMIT should pass
    let res_ok = fw.inspect_statement("SELECT * FROM large_table LIMIT 100;");
    assert!(res_ok.is_ok());
}

#[test]
fn test_block_ddl_prohibits_drop_and_truncate() {
    let fw = create_test_firewall(false, false, true);

    // DROP TABLE should be blocked
    let res = fw.inspect_statement("DROP TABLE production_users;");
    assert!(matches!(res, Err(RuleViolation::ForbiddenDDL(_))));

    // TRUNCATE TABLE should be blocked
    let res_trunc = fw.inspect_statement("TRUNCATE TABLE payments;");
    assert!(matches!(res_trunc, Err(RuleViolation::ForbiddenDDL(_))));

    // Normal SELECT should pass
    let res_ok = fw.inspect_statement("SELECT id FROM payments WHERE id = 1;");
    assert!(res_ok.is_ok());
}
