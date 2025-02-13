use {
    crate::*,
    gluesql_core::{
        executor::{ExecuteError, UpdateError},
        prelude::*,
        translate::TranslateError,
    },
    Value::*,
};

test_case!(update, async move {
    run!(
        r#"
        CREATE TABLE TableA (
            id INTEGER,
            num INTEGER,
            num2 INTEGER,
            name TEXT,
        )"#
    );

    run!(
        r#"
        INSERT INTO TableA (id, num, num2, name)
        VALUES
            (1, 2, 4, "Hello"),
            (1, 9, 5, "World"),
            (3, 4, 7, "Great"),
            (4, 7, 10, "Job");
        "#
    );

    run!(
        r#"
        CREATE TABLE TableB (
            id INTEGER,
            num INTEGER,
            rank INTEGER,
        )"#
    );

    run!(
        r#"
        INSERT INTO TableB (id, num, rank)
        VALUES
            (1, 2, 1),
            (1, 9, 2),
            (3, 4, 3),
            (4, 7, 4);
        "#
    );

    let test_cases = [
        (Ok(Payload::Update(4)), "UPDATE TableA SET id = 2"),
        (
            Ok(select!(id | num; I64 | I64; 2 2; 2 9; 2 4; 2 7)),
            "SELECT id, num FROM TableA",
        ),
        (
            Ok(Payload::Update(1)),
            "UPDATE TableA SET id = 4 WHERE num = 9",
        ),
        (
            Ok(select!(id | num; I64 | I64; 2 2; 4 9; 2 4; 2 7)),
            "SELECT id, num FROM TableA",
        ),
        (
            Ok(Payload::Update(1)),
            "UPDATE TableA SET num2 = (SELECT num FROM TableA WHERE num = 9 LIMIT 1) WHERE num = 9",
        ),
        (
            Ok(select!(id | num | num2; I64 | I64 | I64; 2 2 4; 4 9 9; 2 4 7; 2 7 10)),
            "SELECT id, num, num2 FROM TableA",
        ),
        (
            Ok(Payload::Update(1)),
            "UPDATE TableA SET num2 = (SELECT rank FROM TableB WHERE num = TableA.num) WHERE num = 7",
        ),
        (
            Ok(select!(id | num | num2; I64 | I64 | I64; 2 2 4; 4 9 9; 2 4 7; 2 7 4)),
            "SELECT id, num, num2 FROM TableA",
        ),
        (
            Ok(Payload::Update(1)),
            "UPDATE TableA SET num2 = (SELECT rank FROM TableB WHERE num = TableA.num) WHERE num = (SELECT MIN(num) FROM TableA)",
        ),
        (
            Ok(select!(id | num | num2; I64 | I64 | I64; 2 2 1; 4 9 9; 2 4 7; 2 7 4)),
            "SELECT id, num, num2 FROM TableA",
        ),

    ];

    for (expected, sql) in test_cases {
        test!(expected, sql);
    }

    // Test Error cases for UPDATE
    run!("CREATE TABLE ErrTestTable (id INTEGER);");
    run!("INSERT INTO ErrTestTable (id) VALUES (1),(9);");

    let error_cases = [
        (
            Err(TranslateError::JoinOnUpdateNotSupported.into()),
            "UPDATE TableA INNER JOIN ErrTestTable ON 1 = 1 SET 1 = 1",
        ),
        (
            Err(
                TranslateError::UnsupportedTableFactor("(SELECT * FROM ErrTestTable)".to_owned())
                    .into(),
            ),
            "UPDATE (SELECT * FROM ErrTestTable) SET 1 = 1",
        ),
        (
            Err(TranslateError::CompoundIdentOnUpdateNotSupported(
                "ErrTestTable.id = 1".to_owned(),
            )
            .into()),
            "UPDATE ErrTestTable SET ErrTestTable.id = 1 WHERE id = 1",
        ),
        (
            Err(ExecuteError::TableNotFound("Nothing".to_owned()).into()),
            "UPDATE Nothing SET a = 1;",
        ),
        (
            Err(UpdateError::ColumnNotFound("Foo".to_owned()).into()),
            "UPDATE TableA SET Foo = 1;",
        ),
    ];
    for (expected, sql) in error_cases {
        test!(expected, sql);
    }
});
