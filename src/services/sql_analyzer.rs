use tree_sitter::{Parser, Tree};

#[derive(Debug)]
pub struct SqlQuery {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub start_char: usize,
    pub end_char: usize,
    pub query_text: String,
}

pub struct SqlQueryAnalyzer {
    parser: Parser,
}

impl SqlQueryAnalyzer {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        let language = tree_sitter_sequel::LANGUAGE.into();
        parser.set_language(&language).unwrap();

        Self { parser }
    }

    pub fn detect_queries(&mut self, sql_content: &str) -> Vec<SqlQuery> {
        let tree = match self.parser.parse(sql_content, None) {
            Some(tree) => tree,
            None => return Vec::new(),
        };

        let mut queries = Vec::new();
        self.walk_tree(&tree, sql_content, &mut queries);

        // If tree-sitter didn't find structured queries, try semicolon splitting
        if queries.is_empty() {
            self.fallback_semicolon_split(sql_content, &mut queries);
        }

        queries
    }

    fn walk_tree(&self, tree: &Tree, source: &str, queries: &mut Vec<SqlQuery>) {
        let root_node = tree.root_node();

        let statement_types = [
            "select_statement",
            "insert_statement",
            "update_statement",
            "delete_statement",
            "create_statement",
            "drop_statement",
            "alter_statement",
            "statement",
        ];

        self.traverse_node(&root_node, source, queries, &statement_types);
    }

    fn traverse_node(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        queries: &mut Vec<SqlQuery>,
        statement_types: &[&str],
    ) {
        if statement_types.contains(&node.kind()) {
            let query_text = node
                .utf8_text(source.as_bytes())
                .unwrap_or("")
                .trim()
                .to_string();
            if !query_text.is_empty() && !query_text.starts_with("--") {
                queries.push(SqlQuery {
                    start_byte: node.start_byte(),
                    end_byte: node.end_byte(),
                    start_line: node.start_position().row,
                    end_line: node.end_position().row,
                    start_char: self.byte_to_char_offset(source, node.start_byte()),
                    end_char: self.byte_to_char_offset(source, node.end_byte()),
                    query_text,
                });
            }
        }

        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                self.traverse_node(&child, source, queries, statement_types);
            }
        }
    }

    fn fallback_semicolon_split(&self, source: &str, queries: &mut Vec<SqlQuery>) {
        let mut current_query = String::new();
        let mut query_start_line = 0;
        let mut query_start_char = 0;
        let mut current_char_offset = 0;

        for (line_idx, line) in source.lines().enumerate() {
            if current_query.is_empty() {
                query_start_line = line_idx;
                query_start_char = current_char_offset;
            }

            current_query.push_str(line);
            current_query.push('\n');

            if line.trim().ends_with(';') && !line.trim().starts_with("--") {
                let query_text = current_query.trim().to_string();
                if !query_text.is_empty() {
                    queries.push(SqlQuery {
                        start_byte: query_start_char, // Approximation
                        end_byte: current_char_offset + line.len(),
                        start_line: query_start_line,
                        end_line: line_idx,
                        start_char: query_start_char,
                        end_char: current_char_offset + line.len(),
                        query_text,
                    });
                }
                current_query.clear();
            }

            current_char_offset += line.len() + 1; // +1 for newline
        }

        // Handle case where last query doesn't end with semicolon
        if !current_query.trim().is_empty() {
            queries.push(SqlQuery {
                start_byte: query_start_char,
                end_byte: current_char_offset,
                start_line: query_start_line,
                end_line: source.lines().count().saturating_sub(1),
                start_char: query_start_char,
                end_char: current_char_offset,
                query_text: current_query.trim().to_string(),
            });
        }
    }

    fn byte_to_char_offset(&self, text: &str, byte_offset: usize) -> usize {
        text.char_indices()
            .position(|(i, _)| i >= byte_offset)
            .unwrap_or(text.chars().count())
    }
}
