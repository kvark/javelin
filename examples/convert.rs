use serde::{Deserialize, Serialize};
use std::{env, fs};

#[derive(Hash, PartialEq, Eq, Serialize, Deserialize)]
struct BindSource {
    set: u32,
    binding: u32,
}

#[derive(Serialize, Deserialize)]
struct BindTarget {
    buffer: Option<u8>,
    texture: Option<u8>,
    sampler: Option<u8>,
    mutable: bool,
}

#[derive(Default, Serialize, Deserialize)]
struct Parameters {
    metal_bindings: naga::FastHashMap<BindSource, BindTarget>,
}

fn main() {
    env_logger::init();

    let args = env::args().collect::<Vec<_>>();

    if args.len() <= 1 {
        println!("Call with <input> <output>");
        return;
    }
    let in_ext_pos = args[1].rfind(".").expect("Input has no extension?");
    let module = match &args[1][in_ext_pos..] {
        ".spv" => {
            let input = fs::read(&args[1]).unwrap();
            naga::front::spv::parse_u8_slice(&input).unwrap()
        }
        ".wgsl" => {
            let input = fs::read_to_string(&args[1]).unwrap();
            naga::front::wgsl::parse_str(&input).unwrap()
        }
        #[cfg(feature = "glsl")]
        ".vert" => {
            let input = fs::read_to_string(&args[1]).unwrap();
            naga::front::glsl::parse_str(&input, "main".to_string(), spirv::ExecutionModel::Vertex)
                .unwrap()
        }
        #[cfg(feature = "glsl")]
        ".frag" => {
            let input = fs::read_to_string(&args[1]).unwrap();
            naga::front::glsl::parse_str(
                &input,
                "main".to_string(),
                spirv::ExecutionModel::Fragment,
            )
            .unwrap()
        }
        #[cfg(feature = "glsl")]
        ".comp" => {
            let input = fs::read_to_string(&args[1]).unwrap();
            naga::front::glsl::parse_str(
                &input,
                "main".to_string(),
                spirv::ExecutionModel::GLCompute,
            )
            .unwrap()
        }
        other => panic!("Unknown input extension: {}", other),
    };

    if args.len() <= 2 {
        println!("{:#?}", module);
        return;
    }

    let param_path = std::path::PathBuf::from(&args[1]).with_extension("ron");
    let params = match fs::read_to_string(param_path) {
        Ok(string) => ron::de::from_str(&string).unwrap(),
        Err(_) => Parameters::default(),
    };

    if args[2].ends_with(".metal") {
        use naga::back::msl;
        let mut binding_map = msl::BindingMap::default();
        for (key, value) in params.metal_bindings {
            binding_map.insert(
                msl::BindSource {
                    set: key.set,
                    binding: key.binding,
                },
                msl::BindTarget {
                    buffer: value.buffer,
                    texture: value.texture,
                    sampler: value.sampler,
                    mutable: value.mutable,
                },
            );
        }
        let options = msl::Options {
            binding_map: &binding_map,
        };
        let msl = msl::write_string(&module, options).unwrap();
        fs::write(&args[2], msl).unwrap();
    } else if args[2].ends_with(".spv") {
        use naga::back::spv;

        let debug_flag = args.get(3).map_or(spv::WriterFlags::DEBUG, |arg| {
            if arg.parse().unwrap() {
                spv::WriterFlags::DEBUG
            } else {
                spv::WriterFlags::NONE
            }
        });

        let spv = spv::Writer::new(&module.header, debug_flag).write(&module);

        let bytes = spv
            .iter()
            .fold(Vec::with_capacity(spv.len() * 4), |mut v, w| {
                v.extend_from_slice(&w.to_le_bytes());
                v
            });

        fs::write(&args[2], bytes.as_slice()).unwrap();
    } else {
        panic!("Unknown output: {:?}", args[2]);
    }
}
