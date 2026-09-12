//! **`R_RenderSegLoop` ENTERO**, con todas sus ramas. 2026-09-12.
//!
//! # El instrumento C7 contesto en el Ryzen
//!
//! ```text
//!    [bmo/c7] limpia: vw 320 vh 168 | fc 168 168 168 | cc -1 -1 -1
//!    [bmo/c7] x 0 yl 50 yh 129 | cc -1 fc 168 | mc 0 mf 1 | techo -9/-9 suelo 0/2
//! ```
//!
//! Lo que ENTRA esta bien: el suelo tendria que quedar `130/167`, y queda
//! **`0/2`**. El fallo esta dentro de `R_RenderSegLoop`, al escribir `top` y
//! `bottom`.
//!
//! `sonda_marcar_planos.rs` probo ese bloque RECORTADO y dio bien. Pero desde
//! EL TROQUEL (`52267ab3`) **que locales viven en r12..r15 depende de cuantas
//! veces aparece cada nombre**, y la funcion de verdad usa `texturecolumn`,
//! `mid`, `angle` e `index` muchas mas veces que el recorte. Otro reparto,
//! otro programa. Aqui va entera.

use super::*;

fn fuente(mc: i32, mf: i32) -> String {
    format!(
"typedef unsigned char byte; typedef int fixed_t; typedef unsigned int angle_t;
typedef unsigned int boolean; typedef byte lighttable_t; typedef long long int64_t;
typedef struct {{ fixed_t height; int picnum; int lightlevel; int minx; int maxx;
  byte pad1; byte top[320]; byte pad2; byte pad3; byte bottom[320]; byte pad4; }} visplane_t;
visplane_t planos[2];
visplane_t *ceilingplane; visplane_t *floorplane;
short floorclip[320]; short ceilingclip[320];
int viewheight;
boolean segtextured; boolean markfloor; boolean markceiling; boolean maskedtexture;
int toptexture; int bottomtexture; int midtexture;
int rw_x; int rw_stopx;
angle_t rw_centerangle; fixed_t rw_offset; fixed_t rw_distance;
fixed_t rw_scale; fixed_t rw_scalestep;
fixed_t rw_midtexturemid; fixed_t rw_toptexturemid; fixed_t rw_bottomtexturemid;
fixed_t pixhigh; fixed_t pixlow; fixed_t pixhighstep; fixed_t pixlowstep;
fixed_t topfrac; fixed_t topstep; fixed_t bottomfrac; fixed_t bottomstep;
lighttable_t *walllights[48];
short *maskedtexturecol;
angle_t xtoviewangle[321];
fixed_t finetangent[4096];
lighttable_t *dc_colormap; int dc_x; fixed_t dc_iscale; fixed_t dc_texturemid;
int dc_yl; int dc_yh; byte *dc_source;
byte columna[64]; lighttable_t luz[256];
int columnas;

fixed_t FixedMul(fixed_t a, fixed_t b) {{ return ((int64_t) a * (int64_t) b) >> 16; }}
byte *R_GetColumn(int tex, int col) {{ return columna; }}
void colfunc_de_verdad(void) {{ columnas = columnas + 1; }}
void (*colfunc)(void);

void R_RenderSegLoop (void)
{{
    angle_t		angle;
    unsigned		index;
    int			yl;
    int			yh;
    int			mid;
    fixed_t		texturecolumn;
    int			top;
    int			bottom;

    for ( ; rw_x < rw_stopx ; rw_x++)
    {{
	yl = (topfrac+(1<<12)-1)>>12;
	if (yl < ceilingclip[rw_x]+1)
	    yl = ceilingclip[rw_x]+1;
	if (markceiling)
	{{
	    top = ceilingclip[rw_x]+1;
	    bottom = yl-1;
	    if (bottom >= floorclip[rw_x])
		bottom = floorclip[rw_x]-1;
	    if (top <= bottom)
	    {{
		ceilingplane->top[rw_x] = top;
		ceilingplane->bottom[rw_x] = bottom;
	    }}
	}}
	yh = bottomfrac>>12;
	if (yh >= floorclip[rw_x])
	    yh = floorclip[rw_x]-1;
	if (markfloor)
	{{
	    top = yh+1;
	    bottom = floorclip[rw_x]-1;
	    if (top <= ceilingclip[rw_x])
		top = ceilingclip[rw_x]+1;
	    if (top <= bottom)
	    {{
		floorplane->top[rw_x] = top;
		floorplane->bottom[rw_x] = bottom;
	    }}
	}}
	if (segtextured)
	{{
	    angle = (rw_centerangle + xtoviewangle[rw_x])>>19;
	    texturecolumn = rw_offset-FixedMul(finetangent[angle],rw_distance);
	    texturecolumn >>= 16;
	    index = rw_scale>>12;
	    if (index >=  48 )
		index = 48-1;
	    dc_colormap = walllights[index];
	    dc_x = rw_x;
	    dc_iscale = 0xffffffffu / (unsigned)rw_scale;
	}}
        else
        {{
            texturecolumn = 0;
        }}
	if (midtexture)
	{{
	    dc_yl = yl;
	    dc_yh = yh;
	    dc_texturemid = rw_midtexturemid;
	    dc_source = R_GetColumn(midtexture,texturecolumn);
	    colfunc ();
	    ceilingclip[rw_x] = viewheight;
	    floorclip[rw_x] = -1;
	}}
	else
	{{
	    if (toptexture)
	    {{
		mid = pixhigh>>12;
		pixhigh += pixhighstep;
		if (mid >= floorclip[rw_x])
		    mid = floorclip[rw_x]-1;
		if (mid >= yl)
		{{
		    dc_yl = yl;
		    dc_yh = mid;
		    dc_texturemid = rw_toptexturemid;
		    dc_source = R_GetColumn(toptexture,texturecolumn);
		    colfunc ();
		    ceilingclip[rw_x] = mid;
		}}
		else
		    ceilingclip[rw_x] = yl-1;
	    }}
	    else
	    {{
		if (markceiling)
		    ceilingclip[rw_x] = yl-1;
	    }}
	    if (bottomtexture)
	    {{
		mid = (pixlow+(1<<12)-1)>>12;
		pixlow += pixlowstep;
		if (mid <= ceilingclip[rw_x])
		    mid = ceilingclip[rw_x]+1;
		if (mid <= yh)
		{{
		    dc_yl = mid;
		    dc_yh = yh;
		    dc_texturemid = rw_bottomtexturemid;
		    dc_source = R_GetColumn(bottomtexture,
					    texturecolumn);
		    colfunc ();
		    floorclip[rw_x] = mid;
		}}
		else
		    floorclip[rw_x] = yh+1;
	    }}
	    else
	    {{
		if (markfloor)
		    floorclip[rw_x] = yh+1;
	    }}
	    if (maskedtexture)
	    {{
		maskedtexturecol[rw_x] = texturecolumn;
	    }}
	}}
	rw_scale += rw_scalestep;
	topfrac += topstep;
	bottomfrac += bottomstep;
    }}
}}

int main(void)
{{
    int i;
    viewheight = 168;
    for (i = 0; i < 320; i++) {{ floorclip[i] = viewheight; ceilingclip[i] = -1; }}
    for (i = 0; i < 48; i++) walllights[i] = luz;
    ceilingplane = &planos[0]; floorplane = &planos[1];
    memset(planos[0].top, 0xff, 320); memset(planos[1].top, 0xff, 320);
    markceiling = {mc}; markfloor = {mf}; segtextured = 1; midtexture = 1;
    colfunc = colfunc_de_verdad;
    rw_scale = 72867; rw_scalestep = 0; rw_distance = 9430792;
    topfrac = (49 << 12) + 1; topstep = 0; bottomfrac = 129 << 12; bottomstep = 0;
    rw_x = 0; rw_stopx = 320;
    R_RenderSegLoop();
    printf(\"techo %d/%d suelo %d/%d suelo300 %d/%d | %d %d | %d\",
        planos[0].top[5], planos[0].bottom[5], planos[1].top[5], planos[1].bottom[5],
        planos[1].top[300], planos[1].bottom[300], dc_yl, dc_yh, columnas);
    return 0;
}}
")
}

/// *** Lo que vio C7: sin techo, con suelo. El suelo tiene que quedar 130/167.
#[test]
fn la_funcion_entera_marca_el_suelo_de_c7() {
    assert_eq!(
        run_c(&fuente(0, 1)).trim_end(),
        "techo 255/0 suelo 130/167 suelo300 130/167 | 50 129 | 320",
        "R_RenderSegLoop ENTERO no marca el suelo como DOOM: el metal ve 0/2"
    );
}

/// Y con techo tambien: 0/49.
#[test]
fn la_funcion_entera_marca_techo_y_suelo() {
    assert_eq!(
        run_c(&fuente(1, 1)).trim_end(),
        "techo 0/49 suelo 130/167 suelo300 130/167 | 50 129 | 320",
        "R_RenderSegLoop ENTERO no marca techo y suelo"
    );
}
