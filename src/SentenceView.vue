<script setup lang="ts">
import { computed } from "vue";
import type { Annotation, Sentence } from "./types";
const props = defineProps<{
  sentence: Sentence;
  annotations: Annotation[];
  active: string;
  ruby: boolean;
}>();
const emit = defineEmits<{ choose: [annotation: Annotation] }>();
const pieces = computed(() => {
  const chars = Array.from(props.sentence.text);
  const boundaries = new Set([0, chars.length]);
  props.annotations.forEach((a) => {
    boundaries.add(a.start);
    boundaries.add(a.end);
  });
  const nativeReadings = (props.sentence.readings || []).filter(
    (r) =>
      !props.annotations.some(
        (a) => a.reading && a.start < r.end && a.end > r.start,
      ),
  );
  nativeReadings.forEach((r) => {
    boundaries.add(r.start);
    boundaries.add(r.end);
  });
  const points = [...boundaries].sort((a, b) => a - b);
  return points.slice(0, -1).map((start, i) => {
    const end = points[i + 1];
    const covering = props.annotations.filter(
      (a) => a.start <= start && a.end >= end,
    );
    const annotation =
      covering.find((a) => a.id === props.active) || covering[0];
    return {
      start,
      text: chars.slice(start, end).join(""),
      annotation,
      reading:
        annotation && annotation.start === start && annotation.end === end
          ? annotation.reading
          : nativeReadings.find((r) => r.start === start && r.end === end)
              ?.reading || "",
    };
  });
});
function choose(a: Annotation) {
  if (!window.getSelection()?.toString()) emit("choose", a);
}
</script>
<template>
  <span
    :id="'sentence-' + sentence.id"
    :data-sentence="sentence.id"
    class="sentence"
    ><template v-for="p in pieces" :key="p.start"
      ><span
        v-if="p.annotation"
        class="inline-word"
        :class="{
          active: p.annotation.id === active,
          'grammar-mark': p.annotation.kind === '语法',
        }"
        role="button"
        tabindex="0"
        :aria-label="'查看解释：' + p.annotation.quote"
        @click="choose(p.annotation)"
        @keydown.enter.prevent="emit('choose', p.annotation)"
        @keydown.space.prevent="emit('choose', p.annotation)"
        ><ruby v-if="ruby && p.reading"
          >{{ p.text }}<rt>{{ p.reading }}</rt></ruby
        ><template v-else>{{ p.text }}</template></span
      ><ruby v-else-if="ruby && p.reading"
        >{{ p.text }}<rt>{{ p.reading }}</rt></ruby
      ><template v-else>{{ p.text }}</template></template
    ></span
  >
</template>
