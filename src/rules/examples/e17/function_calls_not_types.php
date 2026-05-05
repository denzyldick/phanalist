<?php

class StringNormalizer
{
    public function normalize(string $value): string
    {
        $value = trim($value);
        $value = strtolower($value);
        $value = str_replace(' ', '-', $value);
        $value = preg_replace('/-+/', '-', $value);
        $value = ltrim($value, '-');
        $value = rtrim($value, '-');
        $value = sprintf('%s', $value);
        $value = mb_substr($value, 0, 120);
        $value = rawurlencode($value);
        $value = html_entity_decode($value);

        return custom_project_helper($value);
    }
}
