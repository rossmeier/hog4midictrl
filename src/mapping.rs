#[derive(Clone)]
pub struct ButtonMapping {
    pub name: String,
    pub note: u8,
    pub vel_on: u8,
    pub vel_off: u8,
}

#[derive(Clone)]
pub struct ControlMapping {
    pub name: String,
    pub id: u8,
}

pub struct Mapping {
    control_mappings: Vec<ControlMapping>,
    button_mappings: Vec<ButtonMapping>,
}

impl Mapping {
    pub fn button_mappings(&self) -> &[ButtonMapping] {
        &self.button_mappings
    }
}

impl Mapping {
    pub fn button_from_name(&self, name: &str) -> Option<&ButtonMapping> {
        return self.button_mappings.iter().find(|x| x.name == name);
    }

    pub fn controller_from_id(&self, id: u8) -> Option<&ControlMapping> {
        return self.control_mappings.iter().find(|x| x.id == id);
    }

    pub fn button_from_note(&self, note: u8) -> Option<&ButtonMapping> {
        return self.button_mappings.iter().find(|x| x.note == note);
    }
}

impl Mapping {
    pub fn apc_mini() -> Self {
        let mut ret = Self {
            control_mappings: Default::default(),
            button_mappings: Default::default(),
        };
        #[rustfmt::skip]
        let button_matrix = [
            ["nextpage", "", "", "", "ewheelbutton/1", "ewheelbutton/2", "ewheelbutton/3", "ewheelbutton/4"],
            ["grandmaster", "", "", "", "intensity", "position", "colour", "beam"],
            ["mainchoose", "live", "scene", "cue", "effect", "time", "group", "fixture"],
            ["assert", "macro", "list", "page", "backspace", "slash", "minus", "plus"],
            ["release", "delete", "move", "copy", "seven", "eight", "nine", "thru"],
            ["mainback", "update", "merge", "record", "four", "five", "six", "full"],
            ["mainhalt", "setup", "goto", "set", "one", "two", "three", "at"],
            ["maingo", "pig", "fan", "open", "zero", "dot", "enter", "enter"],
        ];
        // x and y count from bottom left
        let vel_off = |x, y| {
            if x >= 1 && x < 4 {
                if y < 2 {
                    return 5;
                }
                if y >= 4 {
                    return 5;
                }
            }
            if x >= 4 {
                if y < 5 {
                    return 5;
                }
                if y == 7 {
                    return 5;
                }
            }
            return 3;
        };
        for x in 0..8u8 {
            for y in 0..8u8 {
                let name = button_matrix[7 - y as usize][x as usize].to_string();
                if name.is_empty() {
                    continue;
                }
                ret.button_mappings.push(ButtonMapping {
                    name: name,
                    note: 8 * y + x,
                    vel_on: 127,
                    vel_off: vel_off(x, y),
                });
            }
        }
        let right_column = ["highlight", "blind", "clear", "back", "all", "next", "", ""];
        for y in 0..8u8 {
            let name = right_column[y as usize].to_string();
            if name.is_empty() {
                continue;
            }
            ret.button_mappings.push(ButtonMapping {
                name: name,
                note: 82 + y,
                vel_on: 127,
                vel_off: 0,
            });
        }
        // hardcode faders choose buttons
        for i in 1..=9u8 {
            let note = match i {
                9 => 98,
                x => x + 64 - 1,
            };
            ret.button_mappings.push(ButtonMapping {
                name: format!("choose/{}", i),
                note: note,
                vel_on: 127,
                vel_off: 0,
            });
            ret.control_mappings.push(ControlMapping {
                name: format!("fader/{}", i),
                id: 48 + i - 1,
            });
        }

        return ret;
    }
}
