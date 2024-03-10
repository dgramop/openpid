use indoc::formatdoc;

use crate::prelude::*;

pub fn generate_packet_diagram(name: String, contents: Vec<(String, Option<u32>)>) -> String {

    let mut stuffing = String::new();

    for (idx, (name, size)) in contents.iter().enumerate() {
        let sizestr = match size {
            Some(size) => format!("{} bits", size),
            None => format!("Variable"),
        };

        stuffing.push_str(&formatdoc!("
        {idx}: {sizestr}
        {idx}: {{
          explanation: |md {name} |
          explanation.style.font-size: 55
          width:{scaledsize}

          style.font-size: 40
        }}
        ", scaledsize = match size { 
            Some(size) => size*75,
            None => 1000
        }))
    }

    formatdoc!("

    vars: {{
      d2-config: {{
        layout-engine: elk
        theme-id: 0
      }}
    }}


    {name} {{
        style.font-size: 50
        grid-rows: 1
        grid-gap: 0
        {stuffing}
    }}
    ")
}
