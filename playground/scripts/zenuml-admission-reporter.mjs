import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

const ATTACHMENT_PREFIX = "merman-zenuml-admission:";

export default class ZenUmlAdmissionReporter {
  constructor() {
    this.records = [];
  }

  onTestEnd(test, result) {
    const project = test.parent.project()?.name;
    for (const attachment of result.attachments) {
      if (!attachment.name.startsWith(ATTACHMENT_PREFIX) || !attachment.body) {
        continue;
      }
      this.records.push({
        project,
        status: result.status,
        testTitle: test.title,
        attachment: JSON.parse(attachment.body.toString("utf8")),
      });
    }
  }

  async onEnd(result) {
    const output = process.env.MERMAN_ZENUML_ADMISSION_REPORT;
    if (!output) {
      throw new Error("MERMAN_ZENUML_ADMISSION_REPORT is required.");
    }
    await mkdir(path.dirname(output), { recursive: true });
    await writeFile(
      output,
      `${JSON.stringify({ status: result.status, records: this.records }, null, 2)}\n`,
      "utf8"
    );
  }
}
