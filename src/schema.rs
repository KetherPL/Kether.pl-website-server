// @generated automatically by Diesel CLI.

diesel::table! {
    binds (id) {
        id -> Integer,
        author -> Text,
        text -> Text,
    }
}

diesel::table! {
    bind_suggestions (id) {
        id -> Integer,
        author -> Text,
        text -> Text,
        proposed_by -> Text,
    }
}

diesel::table! {
    bind_votings (id) {
        id -> Integer,
        voter_steam_id -> Text,
        voted_bind_id -> Text,
        vote -> Text,
    }
}

diesel::table! {
    commands (id) {
        id -> Integer,
        command -> Text,
        description -> Text,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    binds,
    bind_suggestions,
    bind_votings,
    commands,
);
