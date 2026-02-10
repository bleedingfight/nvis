#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_data() {
        let data = TableData {
            columns: vec!["id".to_string()],
            rows: vec![],
        };
        let result = generate_bar_chart_data(&data, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_stringids_like_data() {
        let data = TableData {
            columns: vec!["id".to_string(), "value".to_string()],
            rows: vec![
                vec!["0".to_string(), "test1".to_string()],
                vec!["1".to_string(), "test2".to_string()],
                vec!["2".to_string(), "test3".to_string()],
            ],
        };
        let result = generate_bar_chart_data(&data, 0);
        assert_eq!(result.len(), 3);
    }
}
