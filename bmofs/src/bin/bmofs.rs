use std::env;
use std::fs::OpenOptions;
use std::io::{Read, Write, Seek, SeekFrom};
use bmofs::{BlockDevice, BLOCK_SIZE, Superblock, TYPE_FILE, TYPE_DIR};

struct FileBlockDevice {
    file: std::fs::File,
}

impl BlockDevice for FileBlockDevice {
    type Error = std::io::Error;

    fn read_block(&mut self, block_idx: u64, buf: &mut [u8; BLOCK_SIZE]) -> Result<(), Self::Error> {
        self.file.seek(SeekFrom::Start(block_idx * BLOCK_SIZE as u64))?;
        self.file.read_exact(buf)?;
        Ok(())
    }

    fn write_block(&mut self, block_idx: u64, buf: &[u8; BLOCK_SIZE]) -> Result<(), Self::Error> {
        self.file.seek(SeekFrom::Start(block_idx * BLOCK_SIZE as u64))?;
        self.file.write_all(buf)?;
        Ok(())
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        print_usage();
        return;
    }

    let command = args[1].as_str();
    let disk_path = args[2].as_str();

    match command {
        "format" => {
            if args.len() < 4 {
                println!("Uso: bmofs format <disk_path> <total_blocks>");
                return;
            }
            let total_blocks: u64 = args[3].parse().expect("total_blocks debe ser un número entero");
            
            // Crear o abrir el archivo del tamaño especificado
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(disk_path)
                .expect("No se pudo crear/abrir el archivo de disco");
            
            // Rellenar el archivo al tamaño correcto
            file.set_len(total_blocks * BLOCK_SIZE as u64).expect("No se pudo dimensionar el archivo de disco");
            
            let mut dev = FileBlockDevice { file };
            // Formatear con 128 inodes por defecto
            match bmofs::format_volume(&mut dev, total_blocks, 128) {
                Ok(_) => println!("[BMO-FS] Volumen formateado correctamente con {} bloques.", total_blocks),
                Err(e) => println!("[BMO-FS] ERROR: {}", e),
            }
        }
        "ls" => {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(disk_path)
                .expect("No se pudo abrir el archivo de disco");
            
            let mut dev = FileBlockDevice { file };
            
            // Leer superblock
            let mut sb_buf = [0u8; BLOCK_SIZE];
            dev.read_block(0, &mut sb_buf).expect("No se pudo leer el superblock");
            
            let sb: Superblock = unsafe {
                core::ptr::read(sb_buf.as_ptr() as *const Superblock)
            };
            
            if !sb.is_valid() {
                println!("[BMO-FS] ERROR: Estructura o firma de disco BMO-FS no válida");
                return;
            }
            
            // Leer inode raíz (inode 2)
            let root_inode_idx = sb.root_inode;
            let root_inode = bmofs::read_inode(&mut dev, &sb, root_inode_idx).expect("No se pudo leer inode raíz");
            
            println!("Listando directorio raíz (Inode {}):", root_inode_idx);
            println!("{:<10} {:<10} {:<20}", "INODE", "TIPO", "NOMBRE");
            println!("----------------------------------------------");
            
            bmofs::iterate_dir(&mut dev, &root_inode, |inode_num, file_type, name_bytes| {
                let name = String::from_utf8_lossy(name_bytes);
                let type_str = match file_type {
                    TYPE_FILE => "FILE",
                    TYPE_DIR => "DIR",
                    _ => "UNKNOWN",
                };
                println!("{:<10} {:<10} {:<20}", inode_num, type_str, name);
                true
            }).expect("Error recorriendo directorio");
        }
        "add" => {
            if args.len() < 5 {
                println!("Uso: bmofs add <disk_path> <host_file_path> <bmo_dest_name>");
                return;
            }
            let host_file = &args[3];
            let bmo_dest_name = &args[4];
            
            // Leer archivo desde el Host OS
            let file_data = std::fs::read(host_file).expect("No se pudo leer el archivo local del host");
            
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(disk_path)
                .expect("No se pudo abrir el archivo de disco");
            
            let mut dev = FileBlockDevice { file };
            
            // Leer superblock
            let mut sb_buf = [0u8; BLOCK_SIZE];
            dev.read_block(0, &mut sb_buf).expect("No se pudo leer el superblock");
            let sb: Superblock = unsafe { core::ptr::read(sb_buf.as_ptr() as *const Superblock) };
            
            if !sb.is_valid() {
                println!("[BMO-FS] ERROR: Estructura o firma de disco BMO-FS no válida");
                return;
            }
            
            // Asignar un nuevo inode
            let new_inode_idx = bmofs::allocate_inode(&mut dev, &sb).expect("No se pudieron asignar inodes");
            
            // Escribir los datos del archivo en el nuevo inode
            bmofs::write_file_data(&mut dev, &sb, new_inode_idx, &file_data).expect("Error escribiendo los bloques de datos");
            
            // Setear el tipo en el inode
            let mut inode = bmofs::read_inode(&mut dev, &sb, new_inode_idx).unwrap();
            inode.file_type = TYPE_FILE;
            bmofs::write_inode(&mut dev, &sb, new_inode_idx, &inode).unwrap();
            
            // Añadir la entrada al directorio raíz
            bmofs::add_dir_entry(&mut dev, &sb, sb.root_inode, bmo_dest_name, new_inode_idx, TYPE_FILE)
                .expect("Error agregando entrada de directorio");
            
            println!("[BMO-FS] Archivo '{}' agregado exitosamente como '{}' (Inode {}).", host_file, bmo_dest_name, new_inode_idx);
        }
        "get" => {
            if args.len() < 5 {
                println!("Uso: bmofs get <disk_path> <bmo_src_name> <host_dest_path>");
                return;
            }
            let bmo_src_name = &args[3];
            let host_dest_path = &args[4];
            
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(disk_path)
                .expect("No se pudo abrir el archivo de disco");
            
            let mut dev = FileBlockDevice { file };
            
            // Leer superblock
            let mut sb_buf = [0u8; BLOCK_SIZE];
            dev.read_block(0, &mut sb_buf).expect("No se pudo leer el superblock");
            let sb: Superblock = unsafe { core::ptr::read(sb_buf.as_ptr() as *const Superblock) };
            
            if !sb.is_valid() {
                println!("[BMO-FS] ERROR: Estructura o firma de disco BMO-FS no válida");
                return;
            }
            
            // Encontrar el inode del archivo buscando por nombre en el directorio raíz
            let root_inode = bmofs::read_inode(&mut dev, &sb, sb.root_inode).expect("No se pudo leer inode raíz");
            let mut target_inode_num: Option<u32> = None;
            
            bmofs::iterate_dir(&mut dev, &root_inode, |inode_num, file_type, name_bytes| {
                let name = String::from_utf8_lossy(name_bytes);
                if name == bmo_src_name.as_str() && file_type == TYPE_FILE {
                    target_inode_num = Some(inode_num);
                    false // detener búsqueda
                } else {
                    true
                }
            }).unwrap();
            
            let Some(inode_num) = target_inode_num else {
                println!("[BMO-FS] ERROR: No se encontró el archivo '{}' en el directorio raíz", bmo_src_name);
                return;
            };
            
            // Leer inode
            let target_inode = bmofs::read_inode(&mut dev, &sb, inode_num).unwrap();
            let file_size = target_inode.size;
            let mut file_buf = vec![0u8; file_size as usize];
            
            bmofs::read_file_data(&mut dev, &sb, inode_num, &mut file_buf).expect("Error leyendo los datos del archivo");
            
            // Guardar al Host OS
            std::fs::write(host_dest_path, file_buf).expect("No se pudo escribir el archivo local en el host");
            
            println!("[BMO-FS] Archivo '{}' extraído exitosamente a '{}' ({} bytes).", bmo_src_name, host_dest_path, file_size);
        }
        _ => {
            println!("Comando desconocido: {}", command);
            print_usage();
        }
    }
}

fn print_usage() {
    println!("BMO-FS CLI Tool — Uso:");
    println!("  bmofs format <disk_path> <total_blocks>   - Inicializa una imagen de disco con BMO-FS");
    println!("  bmofs ls <disk_path>                      - Lista archivos del directorio raíz");
    println!("  bmofs add <disk_path> <host_file> <name>  - Copia un archivo del host dentro de la imagen");
    println!("  bmofs get <disk_path> <name> <host_dest>  - Extrae un archivo de la imagen al host");
}
