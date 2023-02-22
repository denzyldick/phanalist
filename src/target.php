
 <?php

 class uTesting extends FakeClass
  {
    const I_ = null;
    const hello = null;
    private $a;
    private $fake_variable = null;
    $no_= null, $no_modifier = null;

    public function __construct($o)
    {


      $hello = false;
      if($hello == false){
         $this->no_modifier = 'helloworld';
      }else if ($hello === true){

        $this->no_ = 'hmm';

      }

      $this->fake_variable = 'hellworld';
      return '';
    }

    function test($a){

      if($a){

     }
      $this->does_not_exists();
      return 1;

    }



 }
