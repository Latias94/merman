"""Build the synthetic instance transport fixture with fontTools 4.66.0."""

from pathlib import Path
from tempfile import TemporaryDirectory

from fontTools.designspaceLib import AxisDescriptor, DesignSpaceDocument, SourceDescriptor
from fontTools.feaLib.builder import addOpenTypeFeaturesFromString
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTCollection, TTFont
from fontTools.varLib import build
from fontTools.varLib.featureVars import addFeatureVariations


AXES = (
    ("Weight", "wght", 100, 100, 900),
    ("Optical Size", "opsz", 8, 12, 72),
    ("Custom", "CSTM", 0, 0, 1),
    ("Width", "wdth", 75, 100, 125),
    ("Italic", "ital", 0, 0, 1),
    ("Slant", "slnt", -12, 0, 0),
)


def rectangle(width, height):
    pen = TTGlyphPen(None)
    pen.moveTo((0, 0))
    pen.lineTo((0, height))
    pen.lineTo((width, height))
    pen.lineTo((width, 0))
    pen.closePath()
    return pen.glyph()


def master(path, location):
    names = [".notdef", "A", "V", "A_V", "X", "Z"]
    weight = (location["Weight"] - 100) / 8
    optical = location["Optical Size"] - 12
    custom = location["Custom"] * 60
    width = location["Width"] - 100
    italic = location["Italic"] * 15
    slant = -location["Slant"]
    delta = round(weight + optical + custom + width + italic + slant)
    ascender = 800 + round(weight)
    underline = -90 - round(weight / 2)
    advances = dict(zip(names, [500, 420 + delta, 430 + delta, 720 + delta, 500, 600]))

    font = FontBuilder(1000, isTTF=True)
    font.setupGlyphOrder(names)
    font.setupCharacterMap({ord(name): name for name in ("A", "V", "X", "Z")})
    font.setupGlyf({name: rectangle(advance - 40, 680) for name, advance in advances.items()})
    font.setupHorizontalMetrics({name: (advance, 0) for name, advance in advances.items()})
    font.setupHorizontalHeader(ascent=ascender, descent=-200)
    font.setupNameTable(
        {
            "familyName": "Merman Instance Test",
            "styleName": "Regular",
            "psName": "MermanInstanceTest-Regular",
        }
    )
    font.setupOS2(
        version=4,
        fsSelection=1 << 7,
        sTypoAscender=ascender,
        sTypoDescender=-200,
        usWinAscent=950,
        usWinDescent=200,
    )
    font.setupPost(underlinePosition=underline, underlineThickness=50 + round(weight / 2))
    font.setupCOLR({"Z": [("A", 0)]}, version=0)
    font.setupCPAL([[(0.2, 0.4, 0.8, 1.0)]])
    addOpenTypeFeaturesFromString(font.font, "feature liga { sub A V by A_V; } liga;")
    font.font["head"].created = font.font["head"].modified = 3660681600
    font.font.recalcTimestamp = False
    font.save(path)


def main():
    designspace = DesignSpaceDocument()
    defaults = {name: default for name, _, _, default, _ in AXES}
    locations = [defaults]
    for name, tag, minimum, default, maximum in AXES:
        designspace.addAxis(
            AxisDescriptor(name=name, tag=tag, minimum=minimum, default=default, maximum=maximum)
        )
        locations.extend(defaults | {name: value} for value in (minimum, maximum) if value != default)

    with TemporaryDirectory() as temp:
        for i, location in enumerate(locations):
            path = Path(temp) / f"master-{i}.ttf"
            master(path, location)
            designspace.addSource(SourceDescriptor(path=str(path), location=location))

        variable, _, _ = build(designspace)
        # The default weight shapes A and V as .notdef. Higher weights cover both.
        addFeatureVariations(variable, [([{"wght": (0, 0.1)}], {"A": ".notdef", "V": ".notdef"})])
        variable.recalcTimestamp = False
        collection = TTCollection()
        collection.fonts = [TTFont(Path(temp) / "master-0.ttf", recalcTimestamp=False), variable]
        collection.save(Path(__file__).with_name("instance-transport.ttc"))


if __name__ == "__main__":
    main()
