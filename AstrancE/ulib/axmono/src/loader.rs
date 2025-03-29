
pub struct LoaderManager {
}

trait ElfLoader {
    fn from_elf(&self, elf_path: &str);
}
