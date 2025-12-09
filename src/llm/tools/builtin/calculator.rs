//! Calculator builtin tool for performing simple arithmetic operations

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Arguments for the calculator tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CalculatorArgs {
    /// The mathematical operation to perform: "add", "subtract", "multiply", or "divide"
    pub operation: String,
    /// The first operand
    pub a: f64,
    /// The second operand
    pub b: f64,
}

/// Result from the calculator tool
#[derive(Debug, Serialize)]
pub struct CalculatorResult {
    /// The result of the calculation
    pub result: f64,
    /// The operation that was performed
    pub operation: String,
}

/// Execute a simple calculation
pub fn calculate(args: CalculatorArgs) -> Result<CalculatorResult, String> {
    let result = match args.operation.to_lowercase().as_str() {
        "add" => args.a + args.b,
        "subtract" => args.a - args.b,
        "multiply" => args.a * args.b,
        "divide" => {
            if args.b == 0.0 {
                return Err("Cannot divide by zero".to_string());
            }
            args.a / args.b
        }
        op => return Err(format!("Unknown operation: {}", op)),
    };

    Ok(CalculatorResult {
        result,
        operation: args.operation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_addition() {
        let args = CalculatorArgs {
            operation: "add".to_string(),
            a: 5.0,
            b: 3.0,
        };
        let result = calculate(args).unwrap();
        assert_eq!(result.result, 8.0);
    }

    #[test]
    fn test_subtraction() {
        let args = CalculatorArgs {
            operation: "subtract".to_string(),
            a: 10.0,
            b: 4.0,
        };
        let result = calculate(args).unwrap();
        assert_eq!(result.result, 6.0);
    }

    #[test]
    fn test_multiplication() {
        let args = CalculatorArgs {
            operation: "multiply".to_string(),
            a: 7.0,
            b: 6.0,
        };
        let result = calculate(args).unwrap();
        assert_eq!(result.result, 42.0);
    }

    #[test]
    fn test_division() {
        let args = CalculatorArgs {
            operation: "divide".to_string(),
            a: 15.0,
            b: 3.0,
        };
        let result = calculate(args).unwrap();
        assert_eq!(result.result, 5.0);
    }

    #[test]
    fn test_division_by_zero() {
        let args = CalculatorArgs {
            operation: "divide".to_string(),
            a: 10.0,
            b: 0.0,
        };
        let result = calculate(args);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Cannot divide by zero");
    }

    #[test]
    fn test_unknown_operation() {
        let args = CalculatorArgs {
            operation: "power".to_string(),
            a: 2.0,
            b: 3.0,
        };
        let result = calculate(args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown operation"));
    }
}
