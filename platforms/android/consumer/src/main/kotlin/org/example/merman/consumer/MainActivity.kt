package org.example.merman.consumer

import android.app.Activity
import android.os.Bundle
import android.widget.TextView
import io.merman.Merman

class MainActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val svg = Merman.renderSvg("flowchart TD\nA[Hello] --> B[World]")
        setContentView(TextView(this).apply { text = "Merman rendered ${svg.length} SVG characters" })
    }
}
