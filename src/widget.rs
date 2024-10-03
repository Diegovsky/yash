use crate::{bytes_buf, read_line::cursor, sdbg, utils::BytesBuf};

pub type Pos = crate::Vec2;
pub type Size = Pos;

fn paint_selected(text: &[u8]) -> Vec<u8> {
    [b"\x1b[7m", text, b"\x1B[0m"].concat()
}

pub struct GridStyle {
    pub horizontal_gap: u8,
}

impl Default for GridStyle {
    fn default() -> Self {
        Self {
            horizontal_gap: 2
        }
    }
}

#[derive(Default)]
pub struct GridResponse {
    pub elements_shown: u8,
    pub response: Vec<u8>
}


pub fn grid<T: AsRef<[u8]> + std::fmt::Debug>(
    pos: Pos,
    term_size: Size,
    items: &[T],
    selected: u8,
    style: GridStyle,
) -> GridResponse {
    let mut buf = bytes_buf![cursor::kill_to_term_end(), b"\r\n"];
    // TODO: use sorta square root based algorithm for row count
    let rows = 4;
    let mut item_index = 0u8;
    let mut remaining_width = term_size.x as u8;
    for col in items.chunks(rows as usize) {
        if remaining_width == 0 {
            break
        }
        let mut col_buf = BytesBuf::new();

        let mut col_width = 0;
        for item in col.iter() {
            let item = item.as_ref();

            let item = &item[..item.len().min(remaining_width as usize)];

            if item_index == selected {
                col_buf.push(paint_selected(item));
            } else {
                col_buf.push(item);
            }

            // Move cursor to start of next line
            let item_len = item.len() as u32;
            col_buf.push(cursor::move_left(item_len));
            col_buf.push(b"\n");
            col_width = col_width.max(item_len as u8);
            item_index += 1;
        }
        buf.push(col_buf.join(b""));

        // Move cursor to the start of the next column
        let displacement = col_width + style.horizontal_gap;
        buf.push(cursor::move_up(col.len() as u32));
        buf.push(cursor::move_right(displacement as _));

        remaining_width = remaining_width.saturating_sub(col_width);
    }
    // Move cursor to where it was, hopefully 
    buf.push(b"\r");
    buf.push(cursor::move_up(1));
    buf.push(cursor::move_right(pos.x - 1));
    GridResponse { elements_shown: item_index, response: buf.join(b"") }
}

