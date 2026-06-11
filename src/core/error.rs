pub type CResult<T> = color_eyre::Result<T>;
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("[Child Process Error] command '{command}' cause an error:\n\t{err_msg}")]
    ChildProcess {
        /// Use &'static str to ensure that the command is defined in compile time
        /// Cause this program need root permission, this forbids external command injection
        command: &'static str,
        err_msg: String,
    },

    #[error("An avoidable error occured. Please read the 'note' and 'suggestion' section.")]
    General,

    #[error("Invalid Config")]
    InvalidConfig,
    #[error("[Bug] This might be a bug. Please report it:\n\t{0}")]
    Bug(String),

    #[error(
        "[Duplicated Name] There exists another group named '{0}' or it's the same name as the old one."
    )]
    RenamingDuplicatedName(String),
}

#[inline]
pub fn throw_bug<T: Into<String>, E>(msg: T) -> CResult<E> {
    Err(AppError::Bug(msg.into()).into())
}
#[inline]
pub fn throw_invalid_index<T: Into<String>, E>(index: usize, period: T) -> CResult<E> {
    throw_bug(format!(
        "Invalid index({index}) occurs when {}",
        period.into()
    ))
}
