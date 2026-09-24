use ironhold_core::schema::project::{LogicRulesAsset, LogicRule, StateMachineAsset, FsmEventBinding, Action};
use std::fs;
use std::path::Path;

/// Helper: deserialize a RON string with `implicit_some` enabled — matches runtime loader behaviour.
fn from_str<'de, T: serde::Deserialize<'de>>(s: &'de str) -> Result<T, ron::error::SpannedError> {
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(s)
}

#[test]
fn rules_migration_equivalence() {
    // Walk every directory that has both a rules.ron and the generated state_machine.ron
    let mut test_dirs = Vec::new();
    
    // assets/projects/
    for entry in fs::read_dir("assets/projects").expect("assets/projects exists") {
        let entry = entry.expect("valid entry");
        let path = entry.path();
        if path.is_dir() {
            let rules = path.join("logic/rules.ron");
            let fsm = path.join("logic/state_machine.ron");
            if rules.exists() && fsm.exists() {
                test_dirs.push((path, rules, fsm));
            }
        }
    }
    
    // crates/ironhold_cli/tests/fixtures/
    for entry in fs::read_dir("crates/ironhold_cli/tests/fixtures").expect("fixtures exists") {
        let entry = entry.expect("valid entry");
        let path = entry.path();
        if path.is_dir() {
            let rules = path.join("logic/rules.ron");
            let fsm = path.join("logic/state_machine.ron");
            if rules.exists() && fsm.exists() {
                test_dirs.push((path, rules, fsm));
            }
        }
    }
    
    // Also check for my_custom_rules.ron / my_custom_state_machine.ron in state_machine_path_custom_filename_is_discovered
    let custom_rules = Path::new("crates/ironhold_cli/tests/fixtures/state_machine_path_custom_filename_is_discovered/logic/my_custom_rules.ron");
    let custom_fsm = Path::new("crates/ironhold_cli/tests/fixtures/state_machine_path_custom_filename_is_discovered/logic/my_custom_state_machine.ron");
    if custom_rules.exists() && custom_fsm.exists() {
        test_dirs.push((
            Path::new("crates/ironhold_cli/tests/fixtures/state_machine_path_custom_filename_is_discovered").to_path_buf(),
            custom_rules.to_path_buf(),
            custom_fsm.to_path_buf(),
        ));
    }
    
    println!("Testing equivalence for {} directories", test_dirs.len());
    
    for (project_dir, rules_path, fsm_path) in test_dirs {
        let project_name = project_dir.file_name().unwrap().to_string_lossy();
        
        // Skip deliberately unparseable fixture
        if project_name == "bad_rules_parse_no_cascade" || project_name == "bad_state_machine_parse_no_cascade" {
            println!("  Skipping {} (deliberately unparseable)", project_name);
            // Assert it indeed fails to parse
            let rules_content = fs::read_to_string(&rules_path).unwrap();
            let result: Result<LogicRulesAsset, _> = from_str(&rules_content);
            assert!(result.is_err(), "bad_rules_parse_no_cascade must fail to parse");
            continue;
        }
        
        println!("  Checking {}", project_name);
        
        let rules_content = fs::read_to_string(&rules_path).expect(&format!("read {}", rules_path.display()));
        let fsm_content = fs::read_to_string(&fsm_path).expect(&format!("read {}", fsm_path.display()));
        
        let rules: LogicRulesAsset = from_str(&rules_content).expect(&format!("parse rules.ron for {}", project_name));
        let fsm: StateMachineAsset = from_str(&fsm_content).expect(&format!("parse state_machine.ron for {}", project_name));
        
        // 1. Every parsed rule's `when` is None — the one field the transform cannot carry over
        for (i, rule) in rules.rules.iter().enumerate() {
            assert_eq!(rule.when, None, "rules[{}] in {} has when: {:?} — should be None after migration", i, project_name, rule.when);
        }
        
        // 2. rules.iter().map(|r| (&r.on, &r.do_actions)) == global_on.iter().map(|b| (&b.event, &b.do_actions))
        let rules_bindings: Vec<_> = rules.rules.iter().map(|r| (&r.on, &r.do_actions)).collect();
        let fsm_bindings: Vec<_> = fsm.global_on.iter().map(|b| (&b.event, &b.do_actions)).collect();
        
        assert_eq!(rules_bindings.len(), fsm_bindings.len(), 
            "Rule count mismatch for {}: rules.ron has {} rules, state_machine.ron global_on has {} bindings", 
            project_name, rules_bindings.len(), fsm_bindings.len());
        
        for (i, ((rule_on, rule_actions), (fsm_event, fsm_actions))) in rules_bindings.iter().zip(fsm_bindings.iter()).enumerate() {
            assert_eq!(rule_on, fsm_event, "Event mismatch at index {} in {}: rules.on={} vs fsm.event={}", i, project_name, rule_on, fsm_event);
            assert_eq!(rule_actions, fsm_actions, "Actions mismatch at index {} in {}", i, project_name);
        }
        
        // 3. states.is_empty() && transitions.is_empty() && initial_state.is_empty()
        assert!(fsm.states.is_empty(), "{} state_machine.ron states should be empty (flat file)", project_name);
        assert!(fsm.transitions.is_empty(), "{} state_machine.ron transitions should be empty (flat file)", project_name);
        assert_eq!(fsm.initial_state, "", "{} state_machine.ron initial_state should be empty (flat file)", project_name);
        
        // 4. StateMachineAsset::validate() is Ok
        assert!(fsm.validate().is_ok(), "{} state_machine.ron validate() failed: {:?}", project_name, fsm.validate().err());
    }
}