# JNI registers one-shot, operation-control, and reusable-engine methods by exact class name.
-keep class io.merman.Merman { *; }
-keep class io.merman.MermanOperationControl { *; }
-keep class io.merman.MermanEngine { *; }
-keep class io.merman.MermanOperationResult { *; }
-keep class io.merman.MermanException { *; }

# Native icon-pack preflight resolves this exact class and static method by name.
-keep,allowoptimization class io.merman.MermanJniStrings {
    public static long utf8Length(java.lang.String);
}

# The native host-text callback uses JNI class, constructor, method, and field names directly.
-keep,allowoptimization interface io.merman.MermanTextMeasurer {
    io.merman.MermanTextMeasureResult measure(io.merman.MermanTextMeasureRequest);
}
-keepclassmembers,allowoptimization class * implements io.merman.MermanTextMeasurer {
    io.merman.MermanTextMeasureResult measure(io.merman.MermanTextMeasureRequest);
}
-keep,allowoptimization class io.merman.MermanTextMeasureRequest {
    <init>(java.lang.String,java.lang.String,double,java.lang.String,java.lang.String,java.lang.Double,double,double,double,int,int,int,int,int);
}
-keep,allowoptimization class io.merman.MermanTextMeasureResult {
    int resultKind;
    double width;
    double height;
    double length;
    long lineCount;
    double bboxLeft;
    double bboxRight;
    double rawWidth;
    boolean hasRawWidth;
}
