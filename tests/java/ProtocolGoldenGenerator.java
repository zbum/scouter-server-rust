import java.util.HexFormat;

import scouter.io.DataOutputX;
import scouter.lang.pack.TextPack;
import scouter.lang.value.*;

/** Generates the checked-in golden streams using scouter-common 2.21.3. */
public final class ProtocolGoldenGenerator {
    private static void print(String name, DataOutputX out) {
        System.out.println(name + "=" + HexFormat.of().formatHex(out.toByteArray()));
    }

    public static void main(String[] args) throws Exception {
        DataOutputX values = new DataOutputX();
        values.writeValue(new NullValue());
        values.writeValue(new BooleanValue(true));
        values.writeValue(new DecimalValue(Long.MIN_VALUE));
        values.writeValue(new FloatValue(-12.5f));
        values.writeValue(new DoubleValue(Math.PI));

        DoubleSummary doubles = new DoubleSummary();
        doubles.count = 2; doubles.sum = 4.0; doubles.min = 1.5; doubles.max = 2.5;
        values.writeValue(doubles);
        LongSummary longs = new LongSummary();
        longs.count = 3; longs.sum = -9; longs.min = -7; longs.max = 4;
        values.writeValue(longs);

        values.writeValue(new TextValue("Scouter 한글 🚀"));
        values.writeValue(new TextHashValue(0x12345678));
        values.writeValue(new BlobValue(new byte[] {0, 1, (byte) 0xff}));
        values.writeValue(new IP4Value(new byte[] {127, 0, 0, 1}));

        ListValue list = new ListValue();
        list.add(7L); list.add("list"); list.add(false);
        values.writeValue(list);
        values.writeValue(new IntArray(new int[] {Integer.MIN_VALUE, 0, Integer.MAX_VALUE}));
        values.writeValue(new FloatArray(new float[] {-0.0f, Float.MAX_VALUE}));
        values.writeValue(new TextArray(new String[] {"", "경계"}));
        values.writeValue(new LongArray(new long[] {Long.MIN_VALUE, 0, Long.MAX_VALUE}));

        MapValue map = new MapValue();
        map.put("only", new TextValue("value"));
        values.writeValue(map);
        print("values", values);

        DataOutputX pack = new DataOutputX();
        pack.writePack(new TextPack("SERVICE", -123456789, "UTF-8 한글"));
        print("text_pack", pack);
    }
}
