use plugin_interface;
use plugin_interface::{PluginResult, PluginResultEntry};

#[derive(Debug)]
struct CalcPlugin {

}

impl plugin_interface::Plugin for CalcPlugin {
    fn query(&mut self, query: &str) -> PluginResult {
        match meval::eval_str(&query) {
            Ok(result) => {
                let result = result.to_string();
                PluginResult::Ok(vec![PluginResultEntry::Clip { label : result.clone(),  content : result}])
            }

            Err(err) => {
                PluginResult::Error(format!("{:?}", err))
            }
        }
    }

    fn name(&self) -> &'static str {
        "calc"
    }
}

#[no_mangle]
pub extern "C" fn load(plugin_interface_version : &str) -> Result<Box<dyn plugin_interface::Plugin>, String> {
    if plugin_interface_version == plugin_interface::COMMON_INTERFACE_VERSION {
        Ok(Box::new(CalcPlugin{}))
    } else {
        Err(format!("compatible with: {} but your version is: {}", plugin_interface::COMMON_INTERFACE_VERSION, plugin_interface_version))
    }
}

#[no_mangle]
pub extern "C" fn introduce() -> Box<dyn plugin_interface::Plugin> {
//    let x = Box::new(CalcPlugin{});
//    Box::into_raw(x)
    Box::new(CalcPlugin{})
}

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_interface::*;

    fn assert_ok_result(expression : &str, expected_result : f64) {
        let y = CalcPlugin{}.query(expression);
        if let PluginResult::Ok(result) = y {
            if let PluginResultEntry::Clip{ content, label } = &result[0] {
                assert_eq!(expected_result.to_string(), *content);
                return;
            }
        }

//        panic!("expected ok result got: {:?}", y);
//        if let PluginResult::Ok(x) = CalcPlugin{}.query(expression) {
//
//        }
    }

    #[test]
    fn it_works() {
        assert_ok_result("2+2", 4.0);
        assert_ok_result("8/2*(2+2)", 16.0);
//        assert_eq!(CalcPlugin{}.query("2 + 2"), 4);
    }
}
