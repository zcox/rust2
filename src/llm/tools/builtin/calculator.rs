//! Calculator builtin tool for performing simple arithmetic operations

use rust2_tool_macros::tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The mathematical operation to perform
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
}

/// Arguments for the calculator tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CalculatorArgs {
    /// The mathematical operation to perform
    pub operation: Operation,
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
}

/// Execute a simple calculation
#[tool(
    description = "Perform basic arithmetic operations: add, subtract, multiply, or divide two numbers",
    crate_path = "crate"
)]
pub fn calculate(args: CalculatorArgs) -> Result<CalculatorResult, String> {
    let result = match args.operation {
        Operation::Add => args.a + args.b,
        Operation::Subtract => args.a - args.b,
        Operation::Multiply => args.a * args.b,
        Operation::Divide => {
            if args.b == 0.0 {
                return Err("Cannot divide by zero".to_string());
            }
            args.a / args.b
        }
    };

    Ok(CalculatorResult { result })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_addition() {
        let args = CalculatorArgs {
            operation: Operation::Add,
            a: 5.0,
            b: 3.0,
        };
        let result = calculate(args).unwrap();
        assert_eq!(result.result, 8.0);
    }

    #[test]
    fn test_subtraction() {
        let args = CalculatorArgs {
            operation: Operation::Subtract,
            a: 10.0,
            b: 4.0,
        };
        let result = calculate(args).unwrap();
        assert_eq!(result.result, 6.0);
    }

    #[test]
    fn test_multiplication() {
        let args = CalculatorArgs {
            operation: Operation::Multiply,
            a: 7.0,
            b: 6.0,
        };
        let result = calculate(args).unwrap();
        assert_eq!(result.result, 42.0);
    }

    #[test]
    fn test_division() {
        let args = CalculatorArgs {
            operation: Operation::Divide,
            a: 15.0,
            b: 3.0,
        };
        let result = calculate(args).unwrap();
        assert_eq!(result.result, 5.0);
    }

    #[test]
    fn test_division_by_zero() {
        let args = CalculatorArgs {
            operation: Operation::Divide,
            a: 10.0,
            b: 0.0,
        };
        let result = calculate(args);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Cannot divide by zero");
    }
}
