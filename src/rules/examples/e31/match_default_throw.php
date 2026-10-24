<?php

namespace App;

enum MatchFormat
{
    case Json;
    case Yaml;
}

function parse_format(MatchFormat $format): int
{
    $data = match ($format) {
        MatchFormat::Json => 1,
        MatchFormat::Yaml => 2,
        default => throw new \InvalidArgumentException('Unsupported format'),
    };

    return $data;
}
