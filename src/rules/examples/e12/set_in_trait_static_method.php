<?php

namespace App\Service\e12;

trait Counter {
    public static function setCounter(int $counter): void {
        self::$counter = $counter;
    }
}

class SetInTraitStaticMethod {
    use Counter;

    private static int $counter = 0;
}