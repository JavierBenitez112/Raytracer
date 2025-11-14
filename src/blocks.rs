use raylib::prelude::Vector3;
use crate::cube::Cube;
use crate::material::Material;

pub const GRID_SIZE_X: usize = 9;
pub const GRID_SIZE_Y: usize = 5;
pub const CUBE_SIZE: f32 = 0.5;
pub const CUBE_SPACING: f32 = 0.5;

fn get_material_from_letter(letter: char) -> Option<Material> {
    match letter {
        'R' => Some(Material::new(
            Vector3::new(0.8, 0.2, 0.2),
            10.0,
            [0.9, 0.1, 0.0, 0.0],
            0.0,
            Some("assets/Furnace.png".to_string()),
            None,
        )),
        'B' => Some(Material::new(
            Vector3::new(0.8, 0.4, 0.2),
            20.0,
            [0.8, 0.2, 0.0, 0.0],
            0.0,
            Some("assets/Bookshelf.png".to_string()),
            None,
        )),
        'I' => Some(Material::new(
            Vector3::new(0.4, 0.4, 0.3),
            50.0,
            [0.6, 0.3, 0.1, 0.0],
            0.0,
            Some("assets/obsidiana.png".to_string()),
            None,
        )),
        'G' => Some(Material::new(
            Vector3::new(0.5, 0.8, 1.0), // Azul celeste
            125.0,
            [0.0, 0.3, 0.4, 0.8], // Aumentada reflectividad (albedo[2]) para hacerlo más reflejante
            3.2, // Índice de refracción muy alto para efecto reflejante pronunciado
            Some("assets/glass.png".to_string()),
            None,
        )),
        'Y' => Some(Material::new_emissive(
            Vector3::new(0.9, 0.9, 0.2),
            30.0,
            [0.7, 0.3, 0.0, 0.0],
            0.0,
            Some("assets/glowstone.png".to_string()),
            None,
            0.6, // Intensidad de emisión (reducida)
            Vector3::new(1.0, 0.95, 0.3), // Color de emisión (amarillo más puro)
        )),
        'P' => Some(Material::new(
            Vector3::new(0.8, 0.2, 0.8),
            15.0,
            [0.8, 0.2, 0.0, 0.0],
            0.0,
            Some("assets/chest.png".to_string()),
            None,
        )),
        'C' => Some(Material::new(
            Vector3::new(0.2, 0.8, 0.8),
            25.0,
            [0.7, 0.3, 0.0, 0.0],
            0.0,
            Some("assets/chest.png".to_string()),
            None,
        )),
        'W' => Some(Material::new(
            Vector3::new(0.9, 0.9, 0.9),
            40.0,
            [0.6, 0.4, 0.0, 0.0],
            0.0,
            Some("assets/wood_planks.png".to_string()),
            None,
        )),
        'K' => Some(Material::new(
            Vector3::new(0.1, 0.1, 0.1),
            5.0,
            [0.9, 0.1, 0.0, 0.0],
            0.0,
            Some("assets/obsidiana.png".to_string()),
            None,
        )),
        _ => None,
    }
}

fn create_cube_from_letter(
    letter: char,
    grid_x: usize,
    grid_y: usize,
    layer: usize,
) -> Option<Cube> {
    if let Some(material) = get_material_from_letter(letter) {
        let offset_x = (GRID_SIZE_X as f32 - 1.0) * CUBE_SPACING / 2.0;
        let offset_z = (GRID_SIZE_Y as f32 - 1.0) * CUBE_SPACING / 2.0;
        let x = grid_x as f32 * CUBE_SPACING - offset_x;
        let y = layer as f32 * CUBE_SPACING;
        let z = grid_y as f32 * CUBE_SPACING - offset_z;
        
        Some(Cube {
            center: Vector3::new(x, y, z),
            size: CUBE_SIZE,
            material,
        })
    } else {
        None
    }
}

const LAYER_0: &[&str] = &[
    "WWWWWWWWW",
    "WWWWWWWWW",
    "WWWWWWWWW",
    "WWWWWWWWW",
    "WWWWWWWWW",
];

const LAYER_1: &[&str] = &[
    "         ",
    " BBBBBBB ",
    " GR IYPG ",
    "         ",
    "         ",
];

const LAYER_2: &[&str] = &[
    "         ",
    "  BBBBB  ",
    "     Y   ",
    "         ",
    "         ",
];

const LAYER_3: &[&str] = &[
    "         ",
    "   BBB   ",
    "         ",
    "         ",
    "         ",
];

static LAYERS: &[&[&str]] = &[LAYER_0, LAYER_1, LAYER_2, LAYER_3];

pub fn get_layers() -> &'static [&'static [&'static str]] {
    LAYERS
}

pub fn create_cubes_from_layers(layers: &[&[&str]]) -> Vec<Cube> {
    let mut cubes = Vec::new();
    
    for (layer_idx, layer) in layers.iter().enumerate() {
        for (y, line) in layer.iter().enumerate() {
            if y >= GRID_SIZE_Y {
                break;
            }
            
            let chars: Vec<char> = line.chars().collect();
            for (x, &ch) in chars.iter().enumerate() {
                if x >= GRID_SIZE_X {
                    break;
                }
                
                if let Some(cube) = create_cube_from_letter(ch, x, y, layer_idx) {
                    cubes.push(cube);
                }
            }
        }
    }
    
    cubes
}

