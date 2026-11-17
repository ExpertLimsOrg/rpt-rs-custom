# Convención `qr:` en campos de texto

↑ [[rpt-rs]] · Plan: `docs/superpowers/plans/2026-11-17-qr-prefix-field-text.md`

**Estado:** implementado en `rpt-layout` (convención `qr:`).

## Qué hace

Si el **texto resuelto** del objeto (campo o fórmula) es exactamente:

```text
qr:https://ejemplo.com/item/42
```

el motor dibuja un **QR** en el recuadro del objeto, **una imagen distinta por fila**, en lugar de mostrar el texto.

## Crystal

- SQL con URL pura + **fórmula** en el informe: `"qr:" + {Comando.url}`
- O SQL: `'qr:' || url AS etiqueta` y campo de BD.

Cuadro **cuadrado** en el diseñador (mismo ancho y alto).

## No hace (v1)

- No detecta `qr:` dentro de un párrafo largo.
- No admite prefijo `QR:` en mayúsculas.
- No sustituye objetos blob/imagen nativos.

↑ Diseño anterior (tipo `QrUrlField` solo KDL): ver [[qr-url-field-design]] (histórico).