//! **QUIEN ESCRIBE `top`/`bottom` EN LOS PLANOS: `R_RenderSegLoop`, tal cual.**
//! 2026-09-12.
//!
//! # El instrumento C6 contesto en el Ryzen
//!
//! ```text
//!    [bmo/c6] plano 0..319 alt 0   | sizeof 664 bottom@343 | spans 3
//!    [bmo/c6]   t/b min 0/2 mid 0/2 max 0/2 | borde izq 255/0 der 255/0
//!    (y los otros tres planos, techos Y suelos, igual: 0/2 en toda la anchura)
//! ```
//!
//! La disposicion compilada en DOOM es la del banco (664 y 343). Y C5 dice que
//! el primer muro deja el techo en 0..48 y el suelo en 130..167. O sea que el
//! plano deberia tener `0/48` y `130/167`, y tiene **`0/2` en todos**. Tres
//! filas por plano: los `spans 3` y las `filas 3 de 200`.
//!
//! Aqui va el bloque que marca, con los tipos de DOOM: `short` para los
//! recortes, `fixed_t` para las fracciones, `unsigned int` para los `boolean` y
//! los planos como punteros GLOBALES.

use super::*;

const MARCAR: &str = "typedef unsigned char byte; typedef int fixed_t; typedef unsigned int angle_t;
typedef unsigned int boolean;
typedef struct { fixed_t height; int picnum; int lightlevel; int minx; int maxx;
  byte pad1; byte top[320]; byte pad2; byte pad3; byte bottom[320]; byte pad4; } visplane_t;
visplane_t planos[2];
visplane_t *ceilingplane; visplane_t *floorplane;
short floorclip[320]; short ceilingclip[320];
int viewheight;
boolean segtextured; boolean markfloor; boolean markceiling; boolean maskedtexture;
int toptexture; int bottomtexture; int midtexture;
int rw_x; int rw_stopx;
fixed_t rw_scale; fixed_t rw_scalestep;
fixed_t topfrac; fixed_t topstep; fixed_t bottomfrac; fixed_t bottomstep;
fixed_t pixhigh; fixed_t pixlow; fixed_t pixhighstep; fixed_t pixlowstep;
int dc_yl; int dc_yh; int columnas;

void colfunc_de_verdad(void) { columnas = columnas + 1; }
void (*colfunc)(void);


void R_RenderSegLoop (void)
{
    angle_t angle;
    unsigned index;
    int yl;
    int yh;
    int mid;
    fixed_t texturecolumn;
    int top;
    int bottom;

    for ( ; rw_x < rw_stopx ; rw_x++)
    {
	yl = (topfrac+(1<<12)-1)>>12;
	if (yl < ceilingclip[rw_x]+1)
	    yl = ceilingclip[rw_x]+1;
	if (markceiling)
	{
	    top = ceilingclip[rw_x]+1;
	    bottom = yl-1;
	    if (bottom >= floorclip[rw_x])
		bottom = floorclip[rw_x]-1;
	    if (top <= bottom)
	    {
		ceilingplane->top[rw_x] = top;
		ceilingplane->bottom[rw_x] = bottom;
	    }
	}
	yh = bottomfrac>>12;
	if (yh >= floorclip[rw_x])
	    yh = floorclip[rw_x]-1;
	if (markfloor)
	{
	    top = yh+1;
	    bottom = floorclip[rw_x]-1;
	    if (top <= ceilingclip[rw_x])
		top = ceilingclip[rw_x]+1;
	    if (top <= bottom)
	    {
		floorplane->top[rw_x] = top;
		floorplane->bottom[rw_x] = bottom;
	    }
	}
	if (segtextured)
	{
	    texturecolumn = 0;
	    index = 0;
	    angle = 0;
	}
	else
	{
	    texturecolumn = 0;
	}
	if (midtexture)
	{
	    dc_yl = yl;
	    dc_yh = yh;
	    colfunc ();
	    ceilingclip[rw_x] = viewheight;
	    floorclip[rw_x] = -1;
	}
	else
	{
	    mid = 0;
	}
	rw_scale += rw_scalestep;
	topfrac += topstep;
	bottomfrac += bottomstep;
    }
}

int main(void)
{
    int i;
    viewheight = 168;
    for (i = 0; i < 320; i++) { floorclip[i] = viewheight; ceilingclip[i] = -1; }
    ceilingplane = &planos[0]; floorplane = &planos[1];
    memset(planos[0].top, 0xff, 320); memset(planos[1].top, 0xff, 320);
    markceiling = 1; markfloor = 1; segtextured = 0; midtexture = 1;
    colfunc = colfunc_de_verdad;
    topfrac = 49 << 12; topstep = 0; bottomfrac = 129 << 12; bottomstep = 0;
    rw_x = 0; rw_stopx = 320;
    R_RenderSegLoop();
    printf(\"%d/%d %d/%d %d/%d | %d %d %d | %d\",
        planos[0].top[5], planos[0].bottom[5], planos[1].top[5], planos[1].bottom[5],
        planos[0].top[300], planos[0].bottom[300],
        ceilingclip[5], floorclip[5], dc_yh, columnas);
    return 0;
}
";

/// *** El bloque entero con los numeros de C5: techo 0..48 y suelo 130..167.
#[test]
fn el_bloque_que_marca_los_planos_da_las_filas_de_c5() {
    assert_eq!(
        run_c(MARCAR).trim_end(),
        "0/48 130/167 0/48 | 168 -1 129 | 320",
        "R_RenderSegLoop marca mal los planos: el metal ve 0/2 en todos"
    );
}
