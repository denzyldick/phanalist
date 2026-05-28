<?php

namespace App;

class Undercommented
{
    public function doSomething(): void
    {
        $items = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        $result = 0;
        for ($i = 0; $i < count($items); $i++) {
            $result += $items[$i] * 2;
            if ($result > 20) {
                $result = $result - 10;
            }
        }
        $this->process($result);
    }

    public function process(int $value): void
    {
        $output = $value * 3;
        $this->save($output);
    }

    public function save(int $data): void
    {
        $this->store($data);
    }
}
