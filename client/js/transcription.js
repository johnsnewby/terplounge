export class Transcription {
  sequences = [];

  constructor() {}

  // Add a content segment to the array
  addSegment(segment) {
    console.log(`Adding segment ${JSON.stringify(segment)}`);
    let position = segment.sequence_number;
    if (this.sequences.length < position + 1) {
      this.sequences.length = position + 1;
    } // resize array as appropriate
    let sequence = this.sequences[position];
    if (!sequence) {
      sequence = [];
    }
    sequence.push(segment);
    this.sequences[position] = sequence;
  }

  // get the text

  getText() {
    let content = "";
    for (var sequence of this.sequences) {
      if (!sequence) {
        console.log("Missing sequence, adding ellipses");
        content += " [...] ";
        continue;
      }
      let sortedSegments = sequence.sort(
        (a, b) => a.segment_start - b.segment_start,
      );
      for (var segment of sortedSegments) {
        content += segment.translation;
      }
    }
    return content;
  }

  reset() {
    this.sequences = [];
  }
}
