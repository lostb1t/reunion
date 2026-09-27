//! `VECTORS/V<n>.VEC`: the 3D models INFO-BUY turns on its monitor, one per
//! invention (REUNION.PRG FUN_3d38_0696).
//!
//! ```text
//! per object, until a header of -1 or the end of the file (at most 17):
//!   u16  header (not -1)
//!   u16  vertex count, then i16 x, y, z per vertex (divided by 10 on loading)
//!   u16  edge count, then u8 from, to per edge (1-based vertices)
//!   u16  face count, then 20 bytes per face: 1-based vertex numbers, ended
//!        by 0xff (or the 20th byte)
//! ```

use thiserror::Error;

pub const MAX_OBJECTS: usize = 17;
const FACE_LEN: usize = 20;

#[derive(Debug, Error)]
pub enum VecError {
    #[error("model file ends inside an object")]
    Truncated,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub objects: Vec<Object>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub vertices: Vec<[i16; 3]>,
    pub edges: Vec<[u8; 2]>,
    /// Polygons as 0-based vertex indices.
    pub faces: Vec<Vec<usize>>,
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn u16(&mut self) -> Result<u16, VecError> {
        let bytes = self.data.get(self.pos..self.pos + 2).ok_or(VecError::Truncated)?;
        self.pos += 2;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn bytes(&mut self, n: usize) -> Result<&[u8], VecError> {
        let bytes = self.data.get(self.pos..self.pos + n).ok_or(VecError::Truncated)?;
        self.pos += n;
        Ok(bytes)
    }
}

pub fn decode(data: &[u8]) -> Result<Model, VecError> {
    let mut r = Reader { data, pos: 0 };
    let mut objects = Vec::new();
    while objects.len() < MAX_OBJECTS && r.pos + 2 <= data.len() {
        if r.u16()? == 0xffff {
            break;
        }
        let vertex_count = r.u16()? as usize;
        let vertices = r
            .bytes(vertex_count * 6)?
            .chunks_exact(6)
            .map(|v| {
                // Pascal `div`: truncates toward zero, like Rust.
                [0, 2, 4].map(|i| i16::from_le_bytes([v[i], v[i + 1]]) / 10)
            })
            .collect();
        let edge_count = r.u16()? as usize;
        let edges = r
            .bytes(edge_count * 2)?
            .chunks_exact(2)
            .map(|e| [e[0], e[1]])
            .collect();
        let face_count = r.u16()? as usize;
        let faces = r
            .bytes(face_count * FACE_LEN)?
            .chunks_exact(FACE_LEN)
            .map(|f| {
                f.iter()
                    .take_while(|&&v| v != 0xff)
                    .filter(|&&v| v != 0)
                    .map(|&v| v as usize - 1)
                    .collect()
            })
            .collect();
        objects.push(Object {
            vertices,
            edges,
            faces,
        });
    }
    Ok(Model { objects })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_objects_until_the_end_marker() {
        let mut data = vec![];
        data.extend(1u16.to_le_bytes()); // header
        data.extend(3u16.to_le_bytes());
        for v in [[10i16, 0, 0], [0, 25, 0], [0, 0, -39]] {
            for c in v {
                data.extend(c.to_le_bytes());
            }
        }
        data.extend(1u16.to_le_bytes());
        data.extend([1, 2]);
        data.extend(1u16.to_le_bytes());
        let mut face = vec![1, 2, 3, 0xff];
        face.resize(FACE_LEN, 0);
        data.extend(face);
        data.extend(0xffffu16.to_le_bytes());
        let model = decode(&data).unwrap();
        assert_eq!(model.objects.len(), 1);
        let o = &model.objects[0];
        assert_eq!(o.vertices, [[1, 0, 0], [0, 2, 0], [0, 0, -3]]);
        assert_eq!(o.edges, [[1, 2]]);
        assert_eq!(o.faces, [vec![0, 1, 2]]);
    }
}
