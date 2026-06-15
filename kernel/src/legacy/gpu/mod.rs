//! GPU dedicada experimental.
//!
//! Este módulo queda intencionalmente desconectado del kernel funcional. FastOS
//! usa UEFI GOP/framebuffer como backend estable; cualquier driver acelerado
//! deberá volver como backend opcional, no como dependencia del boot path.
